use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, RwLock},
};

use mlua::{Function, Lua, LuaSerdeExt as _, LuaString, Value};
use serde::Deserialize;

use crate::{errors::RunError, shell};

#[derive(Debug, Clone, Default)]
pub struct TaskMap {
    tasks: Arc<RwLock<Vec<Arc<Task>>>>,
}

impl TaskMap {
    #[allow(unused)]
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
                        return Err(mlua::Error::external(
                            "invalid action: expected string or function",
                        ));
                    }
                };
                table.remove("run")?;
                let metadata = lua.from_value(Value::Table(table))?;
                Ok(Self { id, run, metadata })
            }
            _ => Err(mlua::Error::external(
                "invalid task description: expected string, table or function",
            )),
        }
    }

    pub fn run(
        &self,
        tasks: TaskMap,
        args: impl IntoTaskArgs,
    ) -> Pin<Box<dyn Future<Output = Result<(), RunError>> + Send + 'static>> {
        let args = args.into_task_args(&self.metadata);
        let vars: HashMap<String, String> = std::env::vars().chain(args).collect();
        let run = self.run.clone();
        let depends = self.metadata.depends.clone();
        let working_dir = self.id.directory.clone();
        let name: Arc<str> = self.id.name.as_str().into();

        let fut = async move {
            let depends = depends
                .iter()
                .map(|depend| {
                    tasks.resolve(depend, &working_dir).ok_or_else(|| {
                        RunError::DependencyNotFound {
                            task: name.clone(),
                            search: depend.as_str().into(),
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            futures::future::join_all(depends.into_iter().map(|t| t.run(tasks.clone(), ())))
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;

            env::set_current_dir(&working_dir).map_err(|e| RunError::SetCurrentDir {
                task: name.clone(),
                error: e,
                path: working_dir.as_path().into(),
            })?;

            match run {
                TaskAction::Function(function) => {
                    function.call_async(vars).await.map_err(|e| RunError::Lua {
                        task: name.clone(),
                        error: e,
                    })?
                }

                TaskAction::Shell(original_cmd) => {
                    let cmd = subst::substitute(&original_cmd, &vars).map_err(|e| {
                        RunError::SubstitutionError {
                            task: name.clone(),
                            error: e,
                            command: original_cmd.as_str().into(),
                        }
                    })?;

                    println!("$ {cmd}");

                    let code = shell::run_cmd(&cmd).await.map_err(|e| RunError::Shell {
                        task: name.clone(),
                        error: e,
                        command: original_cmd.as_str().into(),
                    })?;

                    if code != 0 {
                        return Err(RunError::ExitCode {
                            task: name.clone(),
                            code,
                            command: original_cmd.as_str().into(),
                        });
                    }
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
