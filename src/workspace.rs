use std::{
    env,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use lazy_static::lazy_static;
use mlua::{Lua, StdLib};
use tokio::fs;

use crate::{
    builtins::add_builtins,
    errors::{RunError, WorkspaceError},
    task::TaskMap,
};

lazy_static! {
    static ref BAURSAK_FILES: Vec<PathBuf> = vec![
        PathBuf::from("baursak.lua"),
        PathBuf::from(".baursak.lua"),
        PathBuf::from("baursak").join("tasks.lua"),
        PathBuf::from(".baursak").join("tasks.lua"),
    ];
}

pub struct Workspace {
    #[allow(unused)]
    root_dir: PathBuf,

    #[allow(unused)]
    current_dir: PathBuf,

    root: Environment,
}

impl Workspace {
    pub async fn load_from_file(file: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        let file = file.as_ref();
        // TODO: every canonicalize error is reported as "does not exist", which hides EACCES/ENOTDIR (wontfix for now)
        // TODO: SpecifiedScriptDoesNotExist shows the path as typed, but TaskFileIsDir shows the canonicalized absolute path
        let file = fs::canonicalize(file)
            .await
            .map_err(|_| WorkspaceError::SpecifiedScriptDoesNotExist(file.into()))?;

        if fs::metadata(&file).await.is_ok_and(|m| m.is_dir()) {
            return Err(WorkspaceError::TaskFileIsDir(file.into()));
        }

        let parent = file
            .parent()
            .expect("file should always have parent")
            .to_owned();

        Self::load_starting_in(parent, Some(file)).await
    }

    pub async fn load() -> Result<Self, WorkspaceError> {
        let current_path = env::current_dir().map_err(WorkspaceError::GetCurrentDir)?;
        Self::load_starting_in(current_path, None).await
    }

    async fn load_starting_in(
        path: impl AsRef<Path>,
        add_to_configs: Option<PathBuf>,
    ) -> Result<Self, WorkspaceError> {
        let mut configs = Self::walk_dirs_to_root(path.as_ref().to_owned()).await;
        if let Some(file) = add_to_configs {
            configs.push(file);
        }

        if configs.is_empty() {
            return Err(WorkspaceError::NoTaskFileFound);
        }

        let (root_path, current_path) = (configs.first().unwrap(), configs.last().unwrap());

        // TODO: The code to collect all parent configs is here, but only the
        // most shallow config will be used. For now, I will forbid calling
        // tasks from parental configs
        let root = Environment::new().map_err(WorkspaceError::CreatingLuaEnvironment)?;
        root.load_script(&current_path).await?;

        Ok(Self {
            root_dir: root_path.to_owned(),
            // TODO: this is the config file's path, not its directory, so `resolve` never matches on directory (wontfix for now)
            current_dir: current_path.to_owned(),
            root,
        })
    }

    async fn walk_dirs_to_root(mut current_path: PathBuf) -> Vec<PathBuf> {
        let mut configs = Vec::new();

        loop {
            let mut found = None;
            'files: for p in BAURSAK_FILES.iter() {
                let p = current_path.join(p);
                if fs::try_exists(&p).await.unwrap_or(false) {
                    found = Some(p);
                    break 'files;
                }
            }

            if let Some(found) = found
                && configs.last() != Some(&found)
            {
                configs.push(found);
            }

            match current_path.parent() {
                Some(p) => current_path = p.to_owned(),
                None => break,
            }
        }

        configs.reverse();

        configs
    }

    pub fn tasks(&self) -> TaskMap {
        self.root.tasks()
    }

    pub async fn run(&self, name: Option<&str>, args: &[String]) -> Result<(), RunError> {
        let tasks = self.tasks();
        match name {
            None => {
                tasks
                    .first()
                    .ok_or_else(|| RunError::NoTasksDefined)?
                    .run(tasks, args.to_vec())
                    .await?
            }

            Some(name) => {
                tasks
                    .resolve(name, &self.current_dir)
                    .ok_or_else(|| RunError::TaskNotFound { task: name.into() })?
                    .run(tasks, args.to_vec())
                    .await?
            }
        };

        Ok(())
    }
}

pub struct Environment {
    lua: Lua,
    state: State,
}

impl Environment {
    fn new() -> mlua::Result<Self> {
        let state = State::default();
        let lua = Lua::new();
        lua.load_std_libs(StdLib::ALL_SAFE)?;
        lua.set_app_data(state.clone());
        add_builtins(&lua, &lua.globals())?;

        Ok(Self { lua, state })
    }

    async fn load_script(&self, path: impl AsRef<Path>) -> Result<(), WorkspaceError> {
        let path = path.as_ref();
        let parent = path.parent().unwrap();
        env::set_current_dir(parent)
            .map_err(|e| WorkspaceError::SetCurrentDir(e, parent.into()))?;

        let contents = fs::read_to_string(path)
            .await
            .map_err(|e| WorkspaceError::ReadingScript(e, path.into()))?;

        self.lua
            .load(contents)
            .set_name(format!("@{}", path.to_string_lossy()))
            .exec_async()
            .await
            .map_err(|e| WorkspaceError::ExecutingScript(e, path.into()))?;

        Ok(())
    }
}

impl Deref for Environment {
    type Target = State;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for Environment {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

#[derive(Default, Clone)]
pub struct State {
    tasks: TaskMap,
}

impl State {
    pub fn get(lua: &Lua) -> State {
        lua.app_data_ref::<State>()
            .map(|s| s.clone())
            .expect("App data should be in lua state")
    }

    pub fn tasks(&self) -> TaskMap {
        self.tasks.clone()
    }
}
