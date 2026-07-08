use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use tempfile::TempDir;

pub struct RunningSession {
    pub temp: TempDir,
    pub name: String,
    child: Option<Child>,
}

impl RunningSession {
    pub fn start(name: &str) -> Self {
        let temp = TempDir::new().unwrap();
        Self::start_in_temp(name, temp)
    }

    #[allow(dead_code)]
    pub fn start_with_existing_log(name: &str, content: &str) -> Self {
        let temp = TempDir::new().unwrap();
        let log_dir = temp.path().join("logs");
        fs::create_dir_all(&log_dir).unwrap();
        fs::write(log_dir.join(format!("{name}.log")), content).unwrap();
        Self::start_in_temp(name, temp)
    }

    fn start_in_temp(name: &str, temp: TempDir) -> Self {
        let runtime = temp.path().join("run");
        let log_dir = temp.path().join("logs");
        let child = Command::cargo_bin("gatefs")
            .unwrap()
            .arg("run")
            .arg(name)
            .env("GATEFS_RUNTIME_DIR", &runtime)
            .env("GATEFS_LOG_DIR", &log_dir)
            .env_remove("GATEFS_SOCKET")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        wait_for_socket(&runtime.join(format!("{name}.sock")));
        Self {
            temp,
            name: name.to_string(),
            child: Some(child),
        }
    }

    pub fn runtime(&self) -> PathBuf {
        self.temp.path().join("run")
    }

    pub fn log_dir(&self) -> PathBuf {
        self.temp.path().join("logs")
    }

    #[allow(dead_code)]
    pub fn socket_path(&self) -> PathBuf {
        self.runtime().join(format!("{}.sock", self.name))
    }

    pub fn sandbox_cmd(&self) -> Command {
        let mut cmd = gatefs_cmd_for(&self.runtime(), &self.log_dir());
        cmd.arg(&self.name);
        cmd
    }

    pub fn stop(&mut self) {
        let already_exited = self
            .child
            .as_mut()
            .and_then(|child| child.try_wait().ok())
            .flatten()
            .is_some();
        if already_exited || self.child.is_none() {
            return;
        }

        let _ = self.sandbox_cmd().arg("destroy").status();
        self.wait_or_kill(Duration::from_secs(5));
    }

    #[allow(dead_code)]
    pub fn wait_for_exit(&mut self, timeout: Duration) -> std::process::ExitStatus {
        let start = Instant::now();
        let child = self.child.as_mut().unwrap();
        loop {
            match child.try_wait().unwrap() {
                Some(status) => return status,
                None if start.elapsed() > timeout => {
                    child.kill().unwrap();
                    panic!("run process did not exit before timeout");
                }
                None => thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    fn wait_or_kill(&mut self, timeout: Duration) {
        let start = Instant::now();
        let child = self.child.as_mut().unwrap();
        loop {
            if let Ok(Some(_)) = child.try_wait() {
                break;
            }
            if start.elapsed() > timeout {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for RunningSession {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn gatefs_cmd_for(runtime: &Path, log_dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("gatefs").unwrap();
    cmd.env("GATEFS_RUNTIME_DIR", runtime)
        .env("GATEFS_LOG_DIR", log_dir)
        .env_remove("GATEFS_SOCKET");
    cmd
}

pub fn wait_for_socket(path: &Path) {
    wait_for(path, "socket did not appear");
}

pub fn wait_for(path: &Path, message: &str) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("{message}: {}", path.display());
}

#[allow(dead_code)]
pub fn wait_until<F>(timeout: Duration, mut condition: F) -> bool
where
    F: FnMut() -> bool,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}
