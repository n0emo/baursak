use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, RwLock},
};

use anyhow::Context as _;
use mlua::{Function, Lua, LuaSerdeExt as _, LuaString, Value};
use serde::Deserialize;
use tokio::process::Command;

#[derive(Debug, Clone, Default)]
pub struct TaskMap {
    tasks: Arc<RwLock<Vec<Arc<Task>>>>,
}

impl TaskMap {
    pub fn get(&self, id: &TaskId) -> Option<Arc<Task>> {
        self.tasks
            .read()
            .unwrap()
            .iter()
            .find(|t| t.id == *id)
            .cloned()
    }

    pub fn resolve(&self, name: &str, directory: impl AsRef<Path>) -> Option<Arc<Task>> {
        let dir = directory.as_ref();
        let tasks = self.tasks.read().unwrap();
        tasks
            .iter()
            .find(|t| t.id.name == name && t.id.directory == dir)
            .or_else(|| tasks.iter().rev().find(|t| t.id.name == name))
            .cloned()
    }

    pub fn insert(&self, task: Task) {
        self.tasks.write().unwrap().push(Arc::new(task));
    }

    pub fn first(&self) -> Option<Arc<Task>> {
        self.tasks.read().unwrap().first().cloned()
    }

    #[allow(unused)]
    pub async fn run(&self, id: &TaskId, args: impl IntoTaskArgs) -> anyhow::Result<()> {
        self.get(id)
            .context(format!("Task `{name}` does not exists", name = id.name))?
            .run(self.clone(), args)
            .await?;
        Ok(())
    }

    pub async fn run_resolve(
        &self,
        name: &str,
        dir: impl AsRef<Path>,
        args: impl IntoTaskArgs,
    ) -> anyhow::Result<()> {
        self.resolve(name, dir)
            .context(format!("Task `{name}` does not exists"))?
            .run(self.clone(), args)
            .await?;
        Ok(())
    }

    pub async fn run_first(&self) -> anyhow::Result<()> {
        self.first()
            .context("There is no tasks defined".to_string())?
            .run(self.clone(), ())
            .await?;
        Ok(())
    }
}

impl IntoIterator for TaskMap {
    type Item = Arc<Task>;

    type IntoIter = std::vec::IntoIter<Arc<Task>>;

    fn into_iter(self) -> Self::IntoIter {
        self.tasks.read().unwrap().clone().into_iter()
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId {
    pub directory: PathBuf,
    pub name: String,
}

impl TaskId {
    pub fn new(directory: PathBuf, name: String) -> Self {
        Self { directory, name }
    }
}

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
    pub id: TaskId,
    pub run: TaskAction,
    pub metadata: TaskMetadata,
}

impl Task {
    pub fn from_function(id: TaskId, function: Function) -> Self {
        Self {
            id,
            run: TaskAction::Function(function),
            metadata: Default::default(),
        }
    }

    pub fn from_command(id: TaskId, command: LuaString) -> Self {
        Self {
            id,
            run: TaskAction::Shell(command.to_string_lossy()),
            metadata: Default::default(),
        }
    }
}

impl Task {
    pub fn from_lua(value: Value, lua: &Lua, id: TaskId) -> mlua::Result<Self> {
        match value {
            Value::String(cmd) => Ok(Self::from_command(id, cmd)),
            Value::Function(function) => Ok(Self::from_function(id, function)),
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
                Ok(Self { id, run, metadata })
            }
            _ => Err(anyhow::anyhow!("Invalid task: expected string, table or function").into()),
        }
    }

    pub fn run(
        &self,
        tasks: TaskMap,
        args: impl IntoTaskArgs,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>> {
        let args = args.into_task_args(&self.metadata);
        let vars: HashMap<String, String> = std::env::vars().chain(args).collect();
        let run = self.run.clone();
        let depends = self.metadata.depends.clone();
        let working_dir = self.id.directory.clone();

        let fut = async move {
            let depends = depends
                .iter()
                .map(|depend| {
                    tasks
                        .resolve(depend, &working_dir)
                        .context("There is no such task")
                })
                .collect::<Result<Vec<_>, _>>()?;

            futures::future::join_all(depends.into_iter().map(|t| t.run(tasks.clone(), ())))
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;

            env::set_current_dir(working_dir)?;

            match run {
                TaskAction::Function(function) => function.call_async(vars).await?,
                TaskAction::Shell(cmd) => {
                    let cmd = subst::substitute(&cmd, &vars)?;
                    println!("$ {cmd}");
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
}

pub trait IntoTaskArgs {
    fn into_task_args(self, metadata: &TaskMetadata) -> HashMap<String, String>;
}

impl IntoTaskArgs for HashMap<String, String> {
    fn into_task_args(self, _metadata: &TaskMetadata) -> HashMap<String, String> {
        // TODO: check against arguments
        self
    }
}

impl IntoTaskArgs for Vec<String> {
    fn into_task_args(self, metadata: &TaskMetadata) -> HashMap<String, String> {
        metadata
            .args
            .clone()
            .into_iter()
            .zip(self.to_vec())
            .collect()
    }
}

impl IntoTaskArgs for () {
    fn into_task_args(self, _metadata: &TaskMetadata) -> HashMap<String, String> {
        // TODO: check if args required
        HashMap::default()
    }
}
