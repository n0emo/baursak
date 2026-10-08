use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use mlua::{Lua, StdLib};
use tokio::fs;

use crate::{builtins::add_builtins, state::App};

mod builtins;
mod state;
mod task;

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

async fn create_lua() -> anyhow::Result<Lua> {
    let app = App::default();

    let lua = unsafe { Lua::unsafe_new() };
    lua.load_std_libs(StdLib::ALL)?;
    lua.set_app_data(app);
    add_builtins(&lua, &lua.globals())?;

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
    let tasks = app.tasks.read().await.clone();
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
