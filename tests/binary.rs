//! Integration tests for the client and server binaries.

use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::{
    env,
    fs::OpenOptions,
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

fn use_dev(path: &str) -> Result<()> {
    let f = OpenOptions::new().read(true).write(true).open(path)?;

    let mut buf = [1u8; 1024];
    f.read_exact_at(&mut buf, 1024)?;
    // file should have all zeros currently
    assert_eq!(&buf[0..10], &[0u8; 10]);

    f.write_all_at(&[3u8; 2], 1024 * 10)?;
    f.sync_data()?;

    f.read_exact_at(&mut buf, 1024 * 10)?;
    assert_eq!(&buf[0..4], [3, 3, 0, 0]);

    Ok(())
}

fn check_use_dev(path: &str) -> Result<()> {
    let f = OpenOptions::new().read(true).write(true).open(path)?;

    // re-read what should be present after use_dev
    let mut buf = [0u8; 1024];
    f.read_exact_at(&mut buf, 1024 * 10)?;
    assert_eq!(&buf[0..4], [3, 3, 0, 0]);

    Ok(())
}

#[test]
// serialize because both tests connect to the same port
#[serial]
// nbd only works on Linux
#[cfg_attr(not(target_os = "linux"), ignore)]
fn test_connect_to_server() -> Result<()> {
    let dev = "/dev/nbd1";
    if !Path::new(dev).exists() {
        eprintln!("nbd is not set up (run sudo modprobe nbd)");
        return Ok(());
    }

    let mut server = Command::new(exe_path("server"))
        .arg("--mem")
        .args(["--size", "10"])
        .spawn()
        .expect("failed to start server");
    // wait for server to start listening for connections
    sleep(Duration::from_millis(100));

    // client should fork and terminate
    let s = Command::new(exe_path("client")).arg(dev).status()?;
    assert!(s.success(), "client exited with an error {s}");

    Command::new("sudo")
        .args(["chown", &whoami::username(), dev])
        .status()
        .expect("failed to chown");

    use_dev(dev)?;

    Command::new(exe_path("client"))
        .arg("--disconnect")
        .arg(dev)
        .status()?;

    server.kill()?;
    server.wait()?;
    Ok(())
}

#[test]
// serialize because both tests connect to the same port
#[serial]
// nbd only works on Linux
#[cfg_attr(not(target_os = "linux"), ignore)]
fn test_foreground_client() -> Result<()> {
    let dev = "/dev/nbd1";

    if !Path::new(dev).exists() {
        eprintln!("nbd is not set up (run sudo modprobe nbd)");
        return Ok(());
    }

    let mut server = Command::new(exe_path("server"))
        .arg("--mem")
        .args(["--size", "10"])
        .spawn()
        .expect("failed to start server");
    sleep(Duration::from_millis(100));

    let mut client = Command::new(exe_path("client"))
        .arg("--foreground")
        .arg(dev)
        .spawn()?;
    sleep(Duration::from_millis(100));

    Command::new("sudo")
        .args(["chown", &whoami::username(), dev])
        .status()
        .expect("failed to chown");
    use_dev(dev)?;

    Command::new(exe_path("client"))
        .arg("--disconnect")
        .arg(dev)
        .status()?;

    let s = client.wait()?;
    assert!(s.success(), "client --foreground failed: {s}");

    server.kill()?;
    server.wait()?;
    Ok(())
}

#[test]
// serialize because tests connect to the same port
#[serial]
// nbd only works on Linux
#[cfg_attr(not(target_os = "linux"), ignore)]
fn test_concurrent_connections() -> Result<()> {
    let dev = "/dev/nbd1";
    let dev2 = "/dev/nbd2";
    if !Path::new(dev).exists() {
        eprintln!("nbd is not set up (run sudo modprobe nbd)");
        return Ok(());
    }

    let mut server = Command::new(exe_path("server"))
        // .arg("--mem")
        .args(["--size", "10"])
        .spawn()
        .expect("failed to start server");
    // wait for server to start listening for connections
    sleep(Duration::from_millis(100));

    // both clients should be able to connect
    Command::new(exe_path("client")).arg(dev).status()?;
    Command::new(exe_path("client")).arg(dev2).status()?;

    Command::new("sudo")
        .args(["chown", &whoami::username(), dev])
        .status()
        .expect("failed to chown");
    Command::new("sudo")
        .args(["chown", &whoami::username(), dev2])
        .status()
        .expect("failed to chown");

    use_dev(dev)?;
    // both devices have the same underlying storage
    check_use_dev(dev2)?;

    Command::new(exe_path("client"))
        .arg("-d")
        .arg(dev)
        .status()?;
    Command::new(exe_path("client"))
        .arg("-d")
        .arg(dev2)
        .status()?;

    // let the clients disconnect on their own
    server.kill()?;
    server.wait()?;
    Ok(())
}
