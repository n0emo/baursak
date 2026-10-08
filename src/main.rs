use std::{
    collections::HashMap,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, RwLock},
};

use anyhow::Context;
use clap::Parser;
use indexmap::IndexMap;
use mlua::{FromLua, Function, Lua, LuaSerdeExt, LuaString, StdLib, Value};
use serde::Deserialize;
use tokio::{fs, process::Command};

const META_FILE: &str = include_str!("meta.lua");

#[derive(Debug, clap::Parser)]
struct Cli {
    #[arg(long, short, default_value = ".")]
    directory: PathBuf,

    #[arg(long, short, default_value = "baursak.lua")]
    file: PathBuf,

    #[arg()]
    task: Option<String>,

    #[arg()]
    args: Vec<String>,

    #[command(flatten)]
    subcommand: Subcommand,
}

#[derive(clap::Args, Debug, Default)]
#[group(multiple = false)]
struct Subcommand {
    #[arg(long, help_heading = Self::HEADING)]
    list: bool,

    #[arg(long, help_heading = Self::HEADING)]
    definitions: bool,
}

impl Subcommand {
    pub(crate) const HEADING: &str = "Commands";
}

#[derive(Debug, Clone)]
enum TaskAction {
    Function(Function),
    Shell(String),
}

#[derive(Debug, Default, Clone, Deserialize)]
struct TaskMetadata {
    #[serde(default = "Vec::new")]
    args: Vec<String>,

    #[serde(default = "Vec::new")]
    depends: Vec<String>,
}

#[derive(Debug, Clone)]
struct Task {
    run: TaskAction,
    metadata: TaskMetadata,
}

impl Task {
    fn from_function(function: Function) -> Self {
        Self {
            run: TaskAction::Function(function),
            metadata: Default::default(),
        }
    }

    fn from_command(command: LuaString) -> Self {
        Self {
            run: TaskAction::Shell(command.to_string_lossy()),
            metadata: Default::default(),
        }
    }
}

impl Task {
    fn run_with_table(
        &self,
        tasks: &IndexMap<String, Task>,
        args: HashMap<String, String>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>>>> {
        let tasks = tasks.to_owned();
        let vars: HashMap<String, String> = std::env::vars().chain(args).collect();
        let run = self.run.clone();
        let depends = self.metadata.depends.clone();

        let fut = async move {
            let depends = depends
                .iter()
                .map(|depend| tasks.get(depend).context("There is no such task"))
                .collect::<Result<Vec<_>, _>>()?;

            futures::future::join_all(depends.into_iter().map(|t| t.run(&tasks, &[])))
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;

            match run {
                TaskAction::Function(function) => function.call_async(vars).await?,
                TaskAction::Shell(cmd) => {
                    let cmd = subst::substitute(&cmd, &vars)?;
                    Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .spawn()?
                        .wait()
                        .await?;
                }
            };

            Ok(())
        };

        Box::pin(fut)
    }

    fn run(
        &self,
        tasks: &IndexMap<String, Task>,
        args: &[String],
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>>>> {
        let args = self
            .metadata
            .args
            .iter()
            .zip(args)
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect();

        self.run_with_table(tasks, args)
    }
}

impl FromLua for Task {
    fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> {
        match value {
            Value::String(cmd) => Ok(Self::from_command(cmd)),
            Value::Function(function) => Ok(Self::from_function(function)),
            Value::Table(table) => {
                let run = match table.get("run")? {
                    Value::String(cmd) => TaskAction::Shell(cmd.to_string_lossy()),
                    Value::Function(function) => TaskAction::Function(function),
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Invalid task 'run': expected string or function"
                        )
                        .into());
                    }
                };
                table.remove("run")?;
                let metadata = lua.from_value(Value::Table(table))?;
                Ok(Self { run, metadata })
            }
            _ => Err(anyhow::anyhow!("Invalid task: expected string, table or function").into()),
        }
    }
}

#[derive(Default)]
struct App {
    tasks: Arc<RwLock<IndexMap<String, Task>>>,
}

async fn create_lua() -> anyhow::Result<Lua> {
    let app = App::default();

    let lua = unsafe { Lua::unsafe_new() };
    lua.load_std_libs(StdLib::ALL)?;
    lua.set_app_data(app);

    lua.globals().set(
        "task",
        lua.create_function(|lua, (name, task): (String, Task)| {
            let app = lua.app_data_ref::<App>().unwrap();
            let mut tasks = app.tasks.write().unwrap();
            tasks.insert(name, task);
            Ok(())
        })?,
    )?;

    lua.globals().set(
        "sh",
        lua.create_async_function(async |_lua, cmd: Vec<String>| {
            let [prog, args @ ..] = cmd.as_slice() else {
                return Err(anyhow::anyhow!("Expected array with atleast 1 element").into());
            };

            println!("{}", cmd.join(" "));
            let out = Command::new(prog)
                .args(args)
                .spawn()?
                .wait_with_output()
                .await?;
            Ok(out.stdout)
        })?,
    )?;

    lua.globals().set(
        "run",
        lua.create_async_function(
            async |lua, (name, args): (String, Option<HashMap<String, String>>)| {
                let app = lua.app_data_ref::<App>().unwrap();
                let tasks = app.tasks.write().unwrap().clone();
                let task = tasks.get(&name).context("Task does not exists")?;
                task.run_with_table(&tasks, args.unwrap_or_default())
                    .await?;
                Ok(())
            },
        )?,
    )?;

    Ok(lua)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let lua = create_lua().await?;
    let path = cli.directory.join(cli.file);
    let script = std::fs::read(path)?;
    lua.load(script).exec_async().await?;

    let app = lua.app_data_ref::<App>().unwrap();
    let tasks = app.tasks.read().unwrap().clone();
    if cli.subcommand.list {
        println!("Available tasks:");
        for (name, _) in tasks.iter() {
            println!("  {name}")
        }
    } else if cli.subcommand.definitions {
        let dir = cli.directory.join("meta");
        fs::create_dir_all(&dir).await?;
        fs::write(dir.join("baursak.lua"), META_FILE).await?;
        println!("Generated type definitions at {dir:?}");
    } else {
        let task = match cli.task {
            None => tasks.first().map(|(_k, v)| v),
            Some(name) => tasks.get(&name),
        }
        .context("No tasks defined")?;
        task.run(&tasks, &cli.args).await?;
    }

    Ok(())
}
