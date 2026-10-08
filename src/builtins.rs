use std::{collections::HashMap, env};

use anyhow::Context as _;
use mlua::{Lua, Result, Table, Variadic};
use tokio::process::Command;

use crate::{state::App, task::Task};

pub fn add_builtins(lua: &Lua, t: &Table) -> Result<()> {
    t.set("task", lua.create_async_function(self::task)?)?;
    t.set("sh", lua.create_async_function(self::sh)?)?;
    t.set("run", lua.create_async_function(self::run)?)?;
    t.set("is_plat", lua.create_function(self::is_plat)?)?;
    Ok(())
}

async fn task(lua: Lua, (name, task): (String, Task)) -> Result<()> {
    let app = lua.app_data_ref::<App>().unwrap();
    let mut tasks = app.tasks.write().await;
    tasks.insert(name, task);
    Ok(())
}

async fn sh(_lua: Lua, cmd: Vec<String>) -> Result<()> {
    let [prog, args @ ..] = cmd.as_slice() else {
        return Err(anyhow::anyhow!("Expected array with atleast 1 element").into());
    };

    println!("{}", cmd.join(" "));
    Command::new(prog).args(args).spawn()?.wait().await?;

    Ok(())
}

async fn run(lua: Lua, (name, args): (String, Option<HashMap<String, String>>)) -> Result<()> {
    let app = lua.app_data_ref::<App>().unwrap();
    let tasks = app.tasks.write().await.clone();
    let task = tasks.get(&name).context("Task does not exists")?;
    task.run_with_table(&tasks, args.unwrap_or_default())
        .await?;
    Ok(())
}

fn is_plat(_lua: &Lua, platforms: Variadic<String>) -> Result<bool> {
    let mut result = false;

    for p in platforms.iter() {
        result |= match p.as_str() {
            "unix" => cfg!(unix),
            "windows" => cfg!(windows),
            "linux" => cfg!(target_os = "linux") || cfg!(target_os = "android"),
            "macos" => cfg!(target_os = "macos"),
            "bsd" => cfg!(any(
                target_os = "freebsd",
                target_os = "openbsd",
                target_os = "netbsd",
                target_os = "dragonfly",
            )),
            "freebsd" => cfg!(target_os = "freebsd"),
            "openbsd" => cfg!(target_os = "openbsd"),
            "netbsd" => cfg!(target_os = "netbsd"),
            "dragonfly" => cfg!(target_os = "dragonfly"),
            "termux" => {
                cfg!(target_os = "android")
                    && (env::var("TERMUX_VERSION").is_ok()
                        || env::var("PREFIX")
                            .unwrap_or_default()
                            .contains("com.termux"))
            }
            "android" => cfg!(target_os = "android"),
            "solaris" => cfg!(target_os = "solaris"),
            "illumos" => cfg!(target_os = "illumos"),
            "cygwin" => cfg!(target_os = "cygwin"),
            "haiku" => cfg!(target_os = "haiku"),
            "aix" => cfg!(target_os = "aix"),
            "hurd" => cfg!(target_os = "hurd"),
            "redox" => cfg!(target_os = "redox"),
            "nto" => cfg!(target_os = "nto"),
            _ => return Err(anyhow::anyhow!("Unknown platform {p}").into()),
        };
    }

    Ok(result)
}
