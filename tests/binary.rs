//! Integration tests for the client and server binaries.

use std::{
    env,
    path::PathBuf,
    process::{Command, Output},
};

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
        .args(["--help"])
        .output()
        .expect("failed to run server --help");
    let stdout = cmd_stdout(out);
    assert!(stdout.contains("server"));
}
