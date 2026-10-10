// TODO: report syntax error better
// TODO: do not use `set_current_dir` and instead pass current dir to shell and other functions

use std::{
    path::PathBuf,
    process::{ExitCode, Termination},
};

use clap::Parser;
use tokio::fs;

use crate::{errors::BaursakError, workspace::Workspace};

mod builtins;
mod errors;
mod shell;
mod task;
mod workspace;

const META_FILE: &str = include_str!("meta.lua");

#[derive(Debug, clap::Parser)]
struct Cli {
    #[arg(long, short, default_value = ".")]
    directory: PathBuf,

    #[arg(long, short)]
    file: Option<PathBuf>,

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
async fn main() -> ExitCode {
    match run_main().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            error.report()
        }
    }
}

async fn run_main() -> Result<(), BaursakError> {
    let cli = Cli::parse();

    std::env::set_current_dir(&cli.directory).map_err(|e| {
        BaursakError::from_io(e, &format!("error: could not cd into {:?}", cli.directory))
    })?;

    let workspace = match cli.file {
        Some(file) => Workspace::load_from_file(file).await,
        None => Workspace::load().await,
    }
    .map_err(BaursakError::from_workspace)?;

    if cli.subcommand.list {
        println!("Available tasks:");
        for task in workspace.tasks() {
            println!("  {}", task.id.name)
        }
    } else if cli.subcommand.definitions {
        let dir = PathBuf::from("meta");
        fs::create_dir_all(&dir).await.map_err(|e| {
            BaursakError::from_io(e, &format!("error: could not create directory {dir:?}"))
        })?;

        let path = dir.join("baursak.lua");
        fs::write(&path, META_FILE)
            .await
            .map_err(|e| BaursakError::from_io(e, &format!("error: could not write {path:?}")))?;

        // TODO: print the absolute path; "meta" is relative to the config's directory, not the directory bk was run from
        println!("generated type definitions at {dir:?}");
    } else {
        workspace
            .run(cli.task.as_deref(), &cli.args)
            .await
            .map_err(BaursakError::from_run)?;
    }

    Ok(())
}
