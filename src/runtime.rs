//! Runtime directory/socket/log path selection.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::{Error, Result};

pub const ENV_RUNTIME_DIR: &str = "GATEFS_RUNTIME_DIR";
pub const ENV_SOCKET: &str = "GATEFS_SOCKET";
pub const ENV_LOG_DIR: &str = "GATEFS_LOG_DIR";
const QUALIFIER: &str = "net";
const ORGANIZATION: &str = "xz-dev";
const APPLICATION: &str = "gatefs";

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub runtime_dir: PathBuf,
    pub log_dir: PathBuf,
    socket_override: Option<PathBuf>,
}

impl RuntimePaths {
    pub fn discover() -> Result<Self> {
        let runtime_dir = runtime_dir()?;
        ensure_runtime_dir(&runtime_dir)?;
        let log_dir = std::env::var_os(ENV_LOG_DIR)
            .map(PathBuf::from)
            .unwrap_or_else(|| runtime_dir.clone());
        ensure_private_dir(&log_dir)?;
        let socket_override = std::env::var_os(ENV_SOCKET).map(PathBuf::from);
        Ok(Self {
            runtime_dir,
            log_dir,
            socket_override,
        })
    }

    pub fn for_tests(runtime_dir: PathBuf, socket_override: Option<PathBuf>) -> Self {
        Self {
            log_dir: runtime_dir.clone(),
            runtime_dir,
            socket_override,
        }
    }

    pub fn for_tests_with_log_dir(
        runtime_dir: PathBuf,
        log_dir: PathBuf,
        socket_override: Option<PathBuf>,
    ) -> Self {
        Self {
            runtime_dir,
            log_dir,
            socket_override,
        }
    }

    pub fn socket_path(&self, name: &str) -> PathBuf {
        self.socket_override
            .clone()
            .unwrap_or_else(|| self.runtime_dir.join(format!("{name}.sock")))
    }

    pub fn sandbox_log_path(&self, name: &str) -> PathBuf {
        self.log_dir.join(format!("{name}.log"))
    }

    pub fn tmp_mount_dir(&self, name: &str, operation_id: u64) -> PathBuf {
        self.runtime_dir
            .join("tmp")
            .join(format!("{name}-{operation_id}"))
    }
}

pub fn runtime_dir() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os(ENV_RUNTIME_DIR) {
        return Ok(PathBuf::from(value));
    }
    let dirs = project_dirs()?;
    Ok(runtime_dir_from_project_dirs(&dirs))
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
        .ok_or_else(|| Error::msg("could not determine standard project directories for gatefs"))
}

fn runtime_dir_from_project_dirs(dirs: &ProjectDirs) -> PathBuf {
    if let Some(runtime_dir) = dirs.runtime_dir() {
        return runtime_dir.to_path_buf();
    }
    dirs.cache_dir().join("run")
}

pub fn ensure_runtime_dir(path: &Path) -> Result<()> {
    ensure_private_dir(path)?;
    ensure_private_dir(&path.join("tmp"))?;
    Ok(())
}

pub fn ensure_private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn env_runtime_dir_wins() {
        let _guard = env_lock();
        let temp = TempDir::new().unwrap();
        let old_runtime = std::env::var_os(ENV_RUNTIME_DIR);
        unsafe {
            std::env::set_var(ENV_RUNTIME_DIR, temp.path());
        }
        assert_eq!(runtime_dir().unwrap(), temp.path());
        unsafe {
            if let Some(old_runtime) = old_runtime {
                std::env::set_var(ENV_RUNTIME_DIR, old_runtime);
            } else {
                std::env::remove_var(ENV_RUNTIME_DIR);
            }
        }
    }

    #[test]
    fn default_socket_is_per_sandbox() {
        let temp = TempDir::new().unwrap();
        let runtime = RuntimePaths::for_tests(temp.path().to_path_buf(), None);
        assert_eq!(runtime.socket_path("demo"), temp.path().join("demo.sock"));
    }

    #[test]
    fn log_dir_can_be_separate_from_runtime_dir() {
        let runtime_temp = TempDir::new().unwrap();
        let log_temp = TempDir::new().unwrap();
        let runtime = RuntimePaths::for_tests_with_log_dir(
            runtime_temp.path().to_path_buf(),
            log_temp.path().to_path_buf(),
            None,
        );
        assert_eq!(
            runtime.socket_path("demo"),
            runtime_temp.path().join("demo.sock")
        );
        assert_eq!(
            runtime.sandbox_log_path("demo"),
            log_temp.path().join("demo.log")
        );
    }

    #[test]
    fn default_runtime_comes_from_directories_project_dirs() {
        let _guard = env_lock();
        let old_runtime = std::env::var_os(ENV_RUNTIME_DIR);
        unsafe {
            std::env::remove_var(ENV_RUNTIME_DIR);
        }
        let dirs = project_dirs().unwrap();
        let expected = runtime_dir_from_project_dirs(&dirs);
        assert_eq!(runtime_dir().unwrap(), expected);
        unsafe {
            if let Some(old_runtime) = old_runtime {
                std::env::set_var(ENV_RUNTIME_DIR, old_runtime);
            }
        }
    }
}
