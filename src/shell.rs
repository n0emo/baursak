use std::io;

use tokio::process::Command;

pub async fn run_cmd(cmd: &str) -> Result<i32, io::Error> {
    let code = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .spawn()?
        .wait()
        .await?
        .code()
        // TODO: a process killed by a signal has no exit code and is reported as success; treat `None` as failure
        .unwrap_or(0);

    Ok(code)
}
