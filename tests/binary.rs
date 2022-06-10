//! Integration tests for the client and server binaries.

use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::process;
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

fn start_server() -> process::Child {
    let server = Command::new(exe_path("server"))
        .args(["--size", "10"])
        .spawn()
        .expect("failed to start server");
    // wait for server to start listening for connections
    sleep(Duration::from_millis(100));
    server
}

fn stop_server(mut server: process::Child) {
    server.kill().expect("could not kill server process");
    server.wait().expect("waiting for server");
}

fn make_public(path: &str) {
    let s = Command::new("sudo")
        .args(["chmod", "a+rw", path])
        .status()
        .expect("chmod failed");
    assert!(s.success());
}

fn client_connect(dev: &str) {
    let s = Command::new(exe_path("client"))
        .arg(dev)
        .status()
        .expect("client connect failed");
    assert!(s.success());
    // wait for client to establish connection
    sleep(Duration::from_millis(100));
}

fn client_disconnect(dev: &str) {
    let s = Command::new(exe_path("client"))
        .arg("--disconnect")
        .arg(dev)
        .status()
        .expect("client disconnect failed");
    assert!(s.success());
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
    f.sync_all()?;

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

    let server = start_server();

    // client should fork and terminate
    client_connect(dev);
    make_public(dev);
    use_dev(dev)?;
    client_disconnect(dev);

    stop_server(server);
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

    let server = start_server();
    sleep(Duration::from_millis(100));

    let mut client = Command::new(exe_path("client"))
        .arg("--foreground")
        .arg(dev)
        .spawn()?;
    sleep(Duration::from_millis(100));

    make_public(dev);
    use_dev(dev)?;

    client_disconnect(dev);

    let s = client.wait()?;
    assert!(s.success(), "client --foreground failed: {s}");

    stop_server(server);
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

    let server = start_server();

    // both clients should be able to connect
    //
    // XXX: this test is careful to use the device before starting a second
    // connection; nbd-client has a comment about how due to some race, Linux
    // has re-reads the partition table on first open of the device, and it's
    // important to do this before calling NBD_DO_IT.
    //
    // That code has some solution for this that I don't understand (yet).
    client_connect(dev);
    sleep(Duration::from_millis(100));

    make_public(dev);
    use_dev(dev)?;

    client_connect(dev2);
    sleep(Duration::from_millis(100));

    check_use_dev(dev)?;
    // both devices have the same underlying storage
    make_public(dev2);
    check_use_dev(dev2)?;

    client_disconnect(dev);
    client_disconnect(dev2);

    stop_server(server);
    Ok(())
}

#[test]
#[serial]
#[cfg_attr(not(target_os = "linux"), ignore)]
fn test_multiple_connections() -> Result<()> {
    let dev = "/dev/nbd1";
    let dev2 = "/dev/nbd2";
    if !Path::new(dev).exists() {
        eprintln!("nbd is not set up (run sudo modprobe nbd)");
        return Ok(());
    }

    let server = start_server();

    // both clients should be able to connect
    client_connect(dev);
    client_connect(dev2);

    make_public(dev);
    make_public(dev2);

    use_dev(dev)?;
    check_use_dev(dev)?;
    check_use_dev(dev2)?;

    client_disconnect(dev);
    client_disconnect(dev2);

    stop_server(server);
    Ok(())
}
