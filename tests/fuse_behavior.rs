mod common;

use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use assert_cmd::prelude::*;

use common::{RunningSession, wait_until};

fn require_fuse() {
    if std::env::var_os("SANDBOXFS_RUN_FUSE_TESTS").is_none() {
        eprintln!("set SANDBOXFS_RUN_FUSE_TESTS=1 to run real FUSE tests");
        return;
    }
    assert!(
        std::path::Path::new("/dev/fuse").exists(),
        "/dev/fuse is required"
    );
    assert!(
        std::process::Command::new("fusermount3")
            .arg("--version")
            .status()
            .is_ok(),
        "fusermount3 is required"
    );
}

fn fuse_enabled() -> bool {
    std::env::var_os("SANDBOXFS_RUN_FUSE_TESTS").is_some()
}

fn require_command(name: &str) {
    assert!(
        std::process::Command::new(name)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok(),
        "{name} is required"
    );
}

fn session_log(session: &RunningSession) -> String {
    fs::read_to_string(session.log_dir().join(format!("{}.log", session.name))).unwrap()
}

fn assert_log_line_contains(log: &str, parts: &[&str]) {
    assert!(
        log.lines()
            .any(|line| parts.iter().all(|part| line.contains(part))),
        "log did not contain line with {parts:?}:\n{log}"
    );
}

fn lsattr_flags(path: &Path) -> String {
    let output = std::process::Command::new("lsattr")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "lsattr failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    stdout
        .split_whitespace()
        .next()
        .unwrap_or_else(|| panic!("lsattr produced no flags for {}", path.display()))
        .to_string()
}

fn assert_immutable_visible(path: &Path) {
    let flags = lsattr_flags(path);
    assert!(flags.contains('i'), "expected immutable flag in {flags}");
}

fn assert_not_immutable_visible(path: &Path) {
    let flags = lsattr_flags(path);
    assert!(!flags.contains('i'), "unexpected immutable flag in {flags}");
}

#[test]
#[ignore]
fn attach_read_and_read_only_write_error() {
    require_fuse();
    if !fuse_enabled() {
        return;
    }
    let session = RunningSession::start("demo_fuse_read");
    let local = session.temp.path().join("local");
    let mountpoint = session.temp.path().join("mnt");
    fs::create_dir_all(&local).unwrap();
    fs::create_dir_all(&mountpoint).unwrap();
    fs::write(local.join("file"), "hello").unwrap();

    session
        .sandbox_cmd()
        .args(["mount", local.to_str().unwrap(), "/data"])
        .assert()
        .success();
    session
        .sandbox_cmd()
        .args(["attach", mountpoint.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(mountpoint.join("data/file")).unwrap(),
        "hello"
    );
    let err = fs::write(mountpoint.join("data/file"), "new").unwrap_err();
    assert!(matches!(
        err.raw_os_error(),
        Some(libc::EROFS | libc::EACCES | libc::EPERM)
    ));
    assert_eq!(fs::read_to_string(local.join("file")).unwrap(), "hello");
}

#[test]
#[ignore]
fn trusted_chattr_preserves_underlying_flags() {
    require_fuse();
    if !fuse_enabled() {
        return;
    }
    require_command("lsattr");
    require_command("chattr");
    let session = RunningSession::start("demo_fuse_trusted_chattr");
    let local = session.temp.path().join("local");
    let mountpoint = session.temp.path().join("mnt");
    fs::create_dir_all(&local).unwrap();
    fs::create_dir_all(&mountpoint).unwrap();
    fs::write(local.join("file"), "hello").unwrap();

    session
        .sandbox_cmd()
        .args(["mount", local.to_str().unwrap(), "/data"])
        .assert()
        .success();
    session
        .sandbox_cmd()
        .args(["attach", mountpoint.to_str().unwrap()])
        .assert()
        .success();

    let host_flags_before = lsattr_flags(&local.join("file"));
    assert_not_immutable_visible(&mountpoint.join("data/file"));

    session
        .sandbox_cmd()
        .args(["chattr", "+i", "/data/file"])
        .assert()
        .success();

    assert_immutable_visible(&mountpoint.join("data/file"));
    assert_eq!(lsattr_flags(&local.join("file")), host_flags_before);

    let log = session_log(&session);
    assert_log_line_contains(
        &log,
        &[" id=", " trusted ", "path=/data/file CHATTR flags=0x10"],
    );
}

#[test]
#[ignore]
fn trusted_chown_preserves_underlying_owner_and_logs() {
    require_fuse();
    if !fuse_enabled() {
        return;
    }
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("trusted chown FUSE test requires root");
        return;
    }
    require_command("chown");
    let session = RunningSession::start("demo_fuse_trusted_chown");
    let local = session.temp.path().join("local");
    let mountpoint = session.temp.path().join("mnt");
    fs::create_dir_all(&local).unwrap();
    fs::create_dir_all(&mountpoint).unwrap();
    fs::write(local.join("file"), "hello").unwrap();
    let underlying_before = fs::metadata(local.join("file")).unwrap();

    session
        .sandbox_cmd()
        .args(["mount", local.to_str().unwrap(), "/data"])
        .assert()
        .success();
    session
        .sandbox_cmd()
        .args(["attach", mountpoint.to_str().unwrap()])
        .assert()
        .success();
    session
        .sandbox_cmd()
        .args(["chown", "1234:2345", "/data/file"])
        .assert()
        .success();

    use std::os::unix::fs::MetadataExt;
    let mount_metadata = fs::metadata(mountpoint.join("data/file")).unwrap();
    assert_eq!(mount_metadata.uid(), 1234);
    assert_eq!(mount_metadata.gid(), 2345);
    let underlying_after = fs::metadata(local.join("file")).unwrap();
    assert_eq!(underlying_after.uid(), underlying_before.uid());
    assert_eq!(underlying_after.gid(), underlying_before.gid());

    let log = session_log(&session);
    assert_log_line_contains(
        &log,
        &[
            " id=",
            " trusted ",
            "path=/data/file SETATTR uid=1234 gid=2345",
        ],
    );
}

#[test]
#[ignore]
fn trusted_chmod_preserves_underlying_mode() {
    require_fuse();
    if !fuse_enabled() {
        return;
    }
    let session = RunningSession::start("demo_fuse_trusted");
    let local = session.temp.path().join("local");
    let mountpoint = session.temp.path().join("mnt");
    fs::create_dir_all(&local).unwrap();
    fs::create_dir_all(&mountpoint).unwrap();
    fs::write(local.join("file"), "hello").unwrap();

    session
        .sandbox_cmd()
        .args(["mount", local.to_str().unwrap(), "/data"])
        .assert()
        .success();
    session
        .sandbox_cmd()
        .args(["attach", mountpoint.to_str().unwrap()])
        .assert()
        .success();
    session
        .sandbox_cmd()
        .args(["chmod", "444", "/data/file"])
        .assert()
        .success();

    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        fs::metadata(local.join("file"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
    assert_eq!(
        fs::metadata(mountpoint.join("data/file"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o444
    );
}

#[test]
#[ignore]
fn direct_chattr_pending_can_be_allowed() {
    require_fuse();
    if !fuse_enabled() {
        return;
    }
    require_command("lsattr");
    require_command("chattr");
    let session = RunningSession::start("demo_fuse_direct_chattr");
    let local = session.temp.path().join("local");
    let mountpoint = session.temp.path().join("mnt");
    fs::create_dir_all(&local).unwrap();
    fs::create_dir_all(&mountpoint).unwrap();
    fs::write(local.join("file"), "hello").unwrap();
    session
        .sandbox_cmd()
        .args(["mount", local.to_str().unwrap(), "/data"])
        .assert()
        .success();
    session
        .sandbox_cmd()
        .args(["attach", mountpoint.to_str().unwrap()])
        .assert()
        .success();

    let mut child = std::process::Command::new("chattr")
        .args(["+i", mountpoint.join("data/file").to_str().unwrap()])
        .spawn()
        .unwrap();
    assert!(wait_until(Duration::from_secs(3), || {
        session
            .sandbox_cmd()
            .arg("allow")
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).contains("CHATTR flags=0x10"))
            .unwrap_or(false)
    }));
    let pending =
        String::from_utf8(session.sandbox_cmd().arg("allow").output().unwrap().stdout).unwrap();
    let id = pending.split_whitespace().next().unwrap().to_string();
    session
        .sandbox_cmd()
        .args(["allow", &id])
        .assert()
        .success();
    assert!(wait_child(&mut child).success());

    let host_flags_before = lsattr_flags(&local.join("file"));
    assert_immutable_visible(&mountpoint.join("data/file"));
    assert_eq!(lsattr_flags(&local.join("file")), host_flags_before);

    let mut child = std::process::Command::new("chattr")
        .args(["-i", mountpoint.join("data/file").to_str().unwrap()])
        .spawn()
        .unwrap();
    assert!(wait_until(Duration::from_secs(3), || {
        session
            .sandbox_cmd()
            .arg("allow")
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).contains("CHATTR flags=0x0"))
            .unwrap_or(false)
    }));
    let pending =
        String::from_utf8(session.sandbox_cmd().arg("allow").output().unwrap().stdout).unwrap();
    let id = pending.split_whitespace().next().unwrap().to_string();
    session
        .sandbox_cmd()
        .args(["allow", &id])
        .assert()
        .success();
    assert!(wait_child(&mut child).success());

    assert_not_immutable_visible(&mountpoint.join("data/file"));
    assert_eq!(lsattr_flags(&local.join("file")), host_flags_before);
}

#[test]
#[ignore]
fn direct_chmod_pending_can_be_allowed_denied_or_do_nothing() {
    require_fuse();
    if !fuse_enabled() {
        return;
    }
    let session = RunningSession::start("demo_fuse_pending");
    let local = session.temp.path().join("local");
    let mountpoint = session.temp.path().join("mnt");
    fs::create_dir_all(&local).unwrap();
    fs::create_dir_all(&mountpoint).unwrap();
    fs::write(local.join("file"), "hello").unwrap();
    session
        .sandbox_cmd()
        .args(["mount", local.to_str().unwrap(), "/data"])
        .assert()
        .success();
    session
        .sandbox_cmd()
        .args(["attach", mountpoint.to_str().unwrap()])
        .assert()
        .success();

    let mut child = std::process::Command::new("chmod")
        .args(["444", mountpoint.join("data/file").to_str().unwrap()])
        .spawn()
        .unwrap();
    assert!(wait_until(Duration::from_secs(3), || {
        session
            .sandbox_cmd()
            .arg("allow")
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).contains("mode=0444"))
            .unwrap_or(false)
    }));
    let pending =
        String::from_utf8(session.sandbox_cmd().arg("allow").output().unwrap().stdout).unwrap();
    let id = pending.split_whitespace().next().unwrap().to_string();
    session
        .sandbox_cmd()
        .args(["allow", &id])
        .assert()
        .success();
    assert!(wait_child(&mut child).success());

    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        fs::metadata(mountpoint.join("data/file"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o444
    );

    let mut child = std::process::Command::new("chmod")
        .args(["555", mountpoint.join("data/file").to_str().unwrap()])
        .spawn()
        .unwrap();
    assert!(wait_until(Duration::from_secs(3), || {
        session
            .sandbox_cmd()
            .arg("allow")
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).contains("mode=0555"))
            .unwrap_or(false)
    }));
    let pending =
        String::from_utf8(session.sandbox_cmd().arg("allow").output().unwrap().stdout).unwrap();
    let id = pending.split_whitespace().next().unwrap().to_string();
    session
        .sandbox_cmd()
        .args(["allow", "--do-nothing", &id])
        .assert()
        .success();
    assert!(wait_child(&mut child).success());
    assert_eq!(
        fs::metadata(mountpoint.join("data/file"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o444
    );

    let mut child = std::process::Command::new("chmod")
        .args(["600", mountpoint.join("data/file").to_str().unwrap()])
        .spawn()
        .unwrap();
    assert!(wait_until(Duration::from_secs(3), || {
        session
            .sandbox_cmd()
            .arg("allow")
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).contains("mode=0600"))
            .unwrap_or(false)
    }));
    let pending =
        String::from_utf8(session.sandbox_cmd().arg("allow").output().unwrap().stdout).unwrap();
    let id = pending.split_whitespace().next().unwrap().to_string();
    session.sandbox_cmd().args(["deny", &id]).assert().success();
    assert!(!wait_child(&mut child).success());
}

fn wait_child(child: &mut std::process::Child) -> std::process::ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let _ = child.wait().unwrap();
            return status;
        }
        if start.elapsed() > Duration::from_secs(5) {
            child.kill().unwrap();
            let _ = child.wait().unwrap();
            panic!("child did not finish");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
