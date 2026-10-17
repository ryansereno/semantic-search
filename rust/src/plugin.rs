use std::ffi::OsString;
use std::io::{self, Read};
use std::path::Path;
use std::process::{ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(60);

const MAX_OUTPUT: u64 = 10_000_000;

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Text(String),
    Failed(String),
}

pub fn run_once(command: &[String], path: &Path) -> io::Result<Outcome> {
    let mut args = Vec::new();
    for arg in &command[1..] {
        args.push(fill_in(arg, path));
    }
    let mut child = Command::new(&command[0])
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()?;

    //read the output on a second thread
    let stdout = child.stdout.take().expect("stdout is piped");
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let bytes = read_capped(stdout);
        let _ = sender.send(bytes);
    });

    let bytes = match receiver.recv_timeout(TIMEOUT) {
        Ok(bytes) => bytes,
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            let message = format!("timed out after {} s", TIMEOUT.as_secs());
            return Ok(Outcome::Failed(message));
        }
    };
    let status = child.wait()?;
    if !status.success() {
        return Ok(Outcome::Failed(status.to_string()));
    }
    Ok(Outcome::Text(String::from_utf8_lossy(&bytes).into_owned()))
}

fn fill_in(arg: &str, path: &Path) -> OsString {
    let mut filled = OsString::new();
    for (index, part) in arg.split("{path}").enumerate() {
        if index > 0 {
            filled.push(path);
        }
        filled.push(part);
    }
    filled
}

fn read_capped(mut stdout: ChildStdout) -> Vec<u8> {
    let mut bytes = Vec::new();
    let _ = stdout.by_ref().take(MAX_OUTPUT).read_to_end(&mut bytes);
    let _ = io::copy(&mut stdout, &mut io::sink());
    bytes
}
