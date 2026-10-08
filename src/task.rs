use std::{collections::HashMap, pin::Pin};

use anyhow::Context as _;
use indexmap::IndexMap;
use mlua::{FromLua, Function, Lua, LuaSerdeExt as _, LuaString, Value};
use serde::Deserialize;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub enum TaskAction {
    Function(Function),
    Shell(String),
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct TaskMetadata {
    #[serde(default = "Vec::new")]
    pub args: Vec<String>,

    #[serde(default = "Vec::new")]
    pub depends: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub run: TaskAction,
    pub metadata: TaskMetadata,
}

impl Task {
    pub fn from_function(function: Function) -> Self {
        Self {
            run: TaskAction::Function(function),
            metadata: Default::default(),
        }
    }

    pub fn from_command(command: LuaString) -> Self {
        Self {
            run: TaskAction::Shell(command.to_string_lossy()),
            metadata: Default::default(),
        }
    }
}

impl Task {
    pub fn run_with_table(
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

    pub fn run(
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
