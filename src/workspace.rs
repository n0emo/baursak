use std::{
    env,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use lazy_static::lazy_static;
use mlua::{Lua, StdLib};
use normalize_path::NormalizePath as _;
use tokio::fs;

use crate::{builtins::add_builtins, task::TaskMap};

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
    pub async fn load(file: impl AsRef<Path>) -> anyhow::Result<Self> {
        let file = file.as_ref();
        let current_path = env::current_dir()?.join(file.normalize());
        let mut configs = Vec::new();
        println!("{current_path:?}");

        if fs::metadata(&current_path).await.is_ok_and(|m| m.is_file()) {
            configs.push(current_path.clone());
        }

        if let Some(current_path) = current_path.parent() {
            Self::walk_dirs_to_root(current_path.to_owned(), &mut configs).await;
        }

        configs.reverse();
        let root_path = configs.first().context("No baursak file found")?.to_owned();
        let current_path = configs.last().context("No baursak file found")?.to_owned();

        // TODO: The code to collect all parent configs is here, but only the
        // most shallow config will be used. For now, I will forbid calling
        // tasks from parental configs
        let root = Environment::new()?;
        root.load_script(&current_path).await?;

        Ok(Self {
            root_dir: root_path,
            current_dir: current_path,
            root,
        })
    }

    async fn walk_dirs_to_root(mut current_path: PathBuf, configs: &mut Vec<PathBuf>) {
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
    }

    pub fn tasks(&self) -> TaskMap {
        self.root.tasks()
    }

    pub async fn run(&self, name: Option<&str>, args: &[String]) -> anyhow::Result<()> {
        let tasks = self.tasks();
        match name {
            None => tasks.run_first().await?,
            Some(name) => {
                tasks
                    .run_resolve(name, &self.current_dir, args.to_vec())
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

    async fn load_script(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        let parent = path.parent().unwrap();
        env::set_current_dir(parent)?;
        let contents = fs::read_to_string(path).await?;
        self.lua
            .load(contents)
            .set_name(format!("@{}", path.to_string_lossy()))
            .exec_async()
            .await?;
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
