use std::{collections::HashMap, env};

use anyhow::anyhow;
use mlua::{Lua, Result, Table, Value, Variadic};
use tokio::process::Command;

use crate::{
    task::{Task, TaskId},
    workspace::State,
};

pub fn add_builtins(lua: &Lua, t: &Table) -> Result<()> {
    t.set("task", lua.create_async_function(self::task)?)?;
    t.set("sh", lua.create_async_function(self::sh)?)?;
    t.set("run", lua.create_async_function(self::run)?)?;
    t.set("is_plat", lua.create_function(self::is_plat)?)?;
    t.set("has_cmd", lua.create_function(self::has_cmd)?)?;
    t.set("assert_cmd", lua.create_function(self::assert_cmd)?)?;
    Ok(())
}

async fn task(lua: Lua, (name, task): (String, Value)) -> Result<()> {
    let dir = env::current_dir()?;
    let id = TaskId::new(dir, name);
    let task = Task::from_lua(task, &lua, id)?;
    State::get(&lua).tasks().insert(task);
    Ok(())
}

async fn sh(_lua: Lua, cmd: String) -> Result<()> {
    println!("$ {cmd}");

    Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .spawn()?
        .wait()
        .await?;

    Ok(())
}

async fn run(lua: Lua, (name, args): (String, Option<HashMap<String, String>>)) -> Result<()> {
    let s = State::get(&lua);

    s.tasks()
        .run_resolve(&name, env::current_dir()?, args.unwrap_or_default())
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

fn has_cmd(_lua: &Lua, cmds: Variadic<String>) -> Result<bool> {
    let mut result = true;
    for cmd in cmds {
        result &= which::which(cmd).is_ok();
    }
    Ok(result)
}

fn assert_cmd(_lua: &Lua, cmds: Variadic<String>) -> Result<()> {
    let mut not_available = Vec::new();
    for cmd in cmds {
        if which::which(&cmd).is_err() {
            not_available.push(cmd);
        }
    }

    if not_available.is_empty() {
        Ok(())
    } else {
        Err(anyhow!("Commands are not available: {}", not_available.join(",")).into())
    }
}
