use std::path::PathBuf;

use clap::Parser;
use tokio::fs;

use crate::workspace::Workspace;

mod builtins;
mod task;
mod workspace;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    std::env::set_current_dir(&cli.directory)?;
    let workspace = Workspace::load(cli.file).await?;

    if cli.subcommand.list {
        println!("Available tasks:");
        for task in workspace.tasks() {
            println!("  {}", task.id.name)
        }
    } else if cli.subcommand.definitions {
        let dir = cli.directory.join("meta");
        fs::create_dir_all(&dir).await?;
        fs::write(dir.join("baursak.lua"), META_FILE).await?;
        println!("Generated type definitions at {dir:?}");
    } else {
        workspace.run(cli.task.as_deref(), &cli.args).await?;
    }

    Ok(())
}
