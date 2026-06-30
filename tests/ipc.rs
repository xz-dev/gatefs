mod common;

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::Stdio;
use std::time::Duration;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use serde_json::json;
use tempfile::TempDir;

use common::{RunningSession, sandboxfs_cmd_for};

#[test]
fn malformed_ipc_request_returns_error_and_session_keeps_running() {
    let session = RunningSession::start("demo_ipc_malformed");
    let mut stream = UnixStream::connect(session.socket_path()).unwrap();
    stream.write_all(b"not json\n").unwrap();
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).unwrap();
    assert!(line.contains("error") || line.contains("expected"));

    session.sandbox_cmd().arg("mount").assert().success();
}

#[test]
fn socket_override_selects_explicit_session_socket() {
    let temp = TempDir::new().unwrap();
    let runtime = temp.path().join("run");
    let log_dir = temp.path().join("logs");
    let socket = temp.path().join("custom.sock");
    let mut child = std::process::Command::cargo_bin("sandboxfs")
        .unwrap()
        .args(["run", "demo_ipc_socket_override"])
        .env("SANDBOXFS_RUNTIME_DIR", &runtime)
        .env("SANDBOXFS_LOG_DIR", &log_dir)
        .env("SANDBOXFS_SOCKET", &socket)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    common::wait_for_socket(&socket);

    sandboxfs_cmd_for(&runtime, &log_dir)
        .env("SANDBOXFS_SOCKET", &socket)
        .args(["demo_ipc_socket_override", "mount"])
        .assert()
        .success();

    sandboxfs_cmd_for(&runtime, &log_dir)
        .env("SANDBOXFS_SOCKET", &socket)
        .args(["demo_ipc_socket_override", "destroy"])
        .assert()
        .success();
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() >= Duration::from_secs(5) {
            child.kill().unwrap();
            let _ = child.wait().unwrap();
            panic!("session did not exit");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let _ = child.wait().unwrap();
    assert!(status.success());
    assert!(!socket.exists());
}

#[test]
fn invalid_session_name_does_not_leave_stale_socket() {
    let temp = TempDir::new().unwrap();
    let runtime = temp.path().join("run");
    let log_dir = temp.path().join("logs");
    let socket = runtime.join("bad/name.sock");

    sandboxfs_cmd_for(&runtime, &log_dir)
        .args(["run", "bad/name"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid sandbox name"));

    assert!(!socket.exists());
}

#[test]
fn raw_ipc_ping_works() {
    let session = RunningSession::start("demo_ipc_ping");
    let mut stream = UnixStream::connect(session.socket_path()).unwrap();
    serde_json::to_writer(&mut stream, &json!({ "type": "ping" })).unwrap();
    stream.write_all(b"\n").unwrap();
    stream.flush().unwrap();
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).unwrap();
    assert_eq!(line.trim(), r#"{"status":"ok"}"#);
}

#[test]
fn concurrent_pending_viewers_do_not_consume_socket_state() {
    let session = RunningSession::start("demo_ipc_concurrent_viewers");
    let local = session.temp.path().join("local");
    fs::create_dir_all(&local).unwrap();
    fs::write(local.join("file"), "hi").unwrap();
    session
        .sandbox_cmd()
        .args(["mount", local.to_str().unwrap(), "/data"])
        .assert()
        .success();

    let runtime = session.runtime();
    let log_dir = session.log_dir();
    let name = session.name.clone();
    let mut viewers = Vec::new();
    for _ in 0..24 {
        let runtime = runtime.clone();
        let log_dir = log_dir.clone();
        let name = name.clone();
        viewers.push(std::thread::spawn(move || {
            let output = sandboxfs_cmd_for(&runtime, &log_dir)
                .args([name.as_str(), "allow"])
                .output()
                .unwrap();
            assert!(output.status.success());
            assert!(String::from_utf8(output.stdout).unwrap().is_empty());
        }));
    }
    for viewer in viewers {
        viewer.join().unwrap();
    }

    session
        .sandbox_cmd()
        .arg("mount")
        .assert()
        .success()
        .stdout(predicate::str::contains("/data"));
    session
        .sandbox_cmd()
        .arg("metadata")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}
