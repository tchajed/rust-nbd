//! Integration tests for the client and server binaries.

use std::{
    env,
    path::PathBuf,
    process::{Command, Output},
    thread::sleep,
    time::Duration,
};

use color_eyre::Result;
use serial_test::serial;

fn exe_path(name: &str) -> PathBuf {
    let bin_dir = env::current_exe()
        .unwrap()
        .parent()
        .expect("test executable's directory")
        .parent()
        .expect("output directory")
        .to_path_buf();
    bin_dir.join(name)
}

fn cmd_stdout(out: Output) -> String {
    String::from_utf8(out.stdout).expect("non utf-8 output")
}

#[test]
fn test_client_help_flag() {
    let out = Command::new(exe_path("client"))
        .args(["--help"])
        .output()
        .expect("failed to run client --help");
    let stdout = cmd_stdout(out);
    assert!(stdout.contains("client"));
}

#[test]
fn test_server_help_flag() {
    let out = Command::new(exe_path("server"))
        .arg("--help")
        .output()
        .expect("failed to run server --help");
    let stdout = cmd_stdout(out);
    assert!(stdout.contains("server"));
}

#[test]
// serialize because both tests connect to the same port
#[serial]
// nbd only works on Linux
#[cfg_attr(not(target_os = "linux"), ignore)]
fn test_connect_to_server() -> Result<()> {
    let mut server = Command::new(exe_path("server"))
        .arg("--mem")
        .args(["--size", "10"])
        .spawn()
        .expect("failed to start server");
    sleep(Duration::from_millis(100));

    let dev = "/dev/nbd1";

    // client should fork and terminate
    let s = Command::new(exe_path("client"))
        .arg(&dev)
        .status()
        .expect("failed to start client");
    assert!(s.success(), "client exited with an error {s}");

    Command::new(exe_path("client"))
        .arg("--disconnect")
        .arg(&dev)
        .status()?;

    server.kill()?;
    Ok(())
}

#[test]
// serialize because both tests connect to the same port
#[serial]
// nbd only works on Linux
#[cfg_attr(not(target_os = "linux"), ignore)]
fn test_foreground_client() -> Result<()> {
    let mut server = Command::new(exe_path("server"))
        .arg("--mem")
        .args(["--size", "10"])
        .spawn()
        .expect("failed to start server");
    sleep(Duration::from_millis(100));

    let dev = "/dev/nbd1";

    let mut client = Command::new(exe_path("client"))
        .arg("--foreground")
        .arg(&dev)
        .spawn()
        .expect("failed to start client");

    Command::new(exe_path("client"))
        .arg("--disconnect")
        .arg(&dev)
        .status()?;

    let s = client.wait()?;
    assert!(s.success(), "client --foreground failed: {s}");

    server.kill()?;
    Ok(())
}
