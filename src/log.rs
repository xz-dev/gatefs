//! Runtime operation log helpers.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct LogWriterHandle {
    sender: mpsc::Sender<LogCommand>,
}

#[derive(Debug)]
pub struct LogWriter {
    handle: LogWriterHandle,
    join: Option<std::thread::JoinHandle<()>>,
}

type LogReply = mpsc::Sender<Result<()>>;

#[derive(Debug)]
enum LogCommand {
    Reset {
        path: PathBuf,
        reply: LogReply,
    },
    Append {
        path: PathBuf,
        line: String,
        reply: LogReply,
    },
    Remove {
        path: PathBuf,
        reply: LogReply,
    },
    Shutdown {
        reply: LogReply,
    },
}

impl Default for LogWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl LogWriter {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let join = std::thread::spawn(move || run_log_writer(receiver));
        Self {
            handle: LogWriterHandle { sender },
            join: Some(join),
        }
    }

    pub fn handle(&self) -> LogWriterHandle {
        self.handle.clone()
    }

    pub fn shutdown(mut self) -> Result<()> {
        self.handle.shutdown()?;
        if let Some(join) = self.join.take() {
            join.join()
                .map_err(|_| Error::msg("log writer thread panicked"))?;
        }
        Ok(())
    }
}

impl Drop for LogWriter {
    fn drop(&mut self) {
        if let Some(join) = self.join.take() {
            let _ = self.handle.shutdown();
            let _ = join.join();
        }
    }
}

impl LogWriterHandle {
    pub fn reset(&self, path: impl AsRef<Path>) -> Result<()> {
        self.request(|reply| LogCommand::Reset {
            path: path.as_ref().to_path_buf(),
            reply,
        })
    }

    pub fn append(&self, path: impl AsRef<Path>, line: impl AsRef<str>) -> Result<()> {
        self.request(|reply| LogCommand::Append {
            path: path.as_ref().to_path_buf(),
            line: line.as_ref().to_string(),
            reply,
        })
    }

    pub fn remove(&self, path: impl AsRef<Path>) -> Result<()> {
        self.request(|reply| LogCommand::Remove {
            path: path.as_ref().to_path_buf(),
            reply,
        })
    }

    fn shutdown(&self) -> Result<()> {
        self.request(|reply| LogCommand::Shutdown { reply })
    }

    fn request(&self, command: impl FnOnce(LogReply) -> LogCommand) -> Result<()> {
        let (reply, receiver) = mpsc::channel();
        self.sender
            .send(command(reply))
            .map_err(|_| Error::msg("log writer stopped"))?;
        receiver
            .recv()
            .map_err(|_| Error::msg("log writer stopped"))?
    }
}

fn run_log_writer(receiver: mpsc::Receiver<LogCommand>) {
    for command in receiver {
        match command {
            LogCommand::Reset { path, reply } => {
                let _ = reply.send(reset_log_file(&path));
            }
            LogCommand::Append { path, line, reply } => {
                let _ = reply.send(append_log_file(&path, &line));
            }
            LogCommand::Remove { path, reply } => {
                let _ = reply.send(remove_log_file(&path));
            }
            LogCommand::Shutdown { reply } => {
                let _ = reply.send(Ok(()));
                break;
            }
        }
    }
}

pub fn reset_log(path: &Path) -> Result<()> {
    default_writer().reset(path)
}

pub fn remove_log(path: &Path) -> Result<()> {
    default_writer().remove(path)
}

pub fn append_log(path: &Path, line: impl AsRef<str>) -> Result<()> {
    default_writer().append(path, line)
}

fn default_writer() -> &'static LogWriterHandle {
    static WRITER: OnceLock<LogWriterHandle> = OnceLock::new();
    WRITER.get_or_init(|| {
        let writer = Box::leak(Box::new(LogWriter::new()));
        writer.handle()
    })
}

fn reset_log_file(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, b"")?;
    Ok(())
}

fn remove_log_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn append_log_file(path: &Path, line: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

pub fn format_timestamp(time: SystemTime) -> String {
    let duration = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0));
    let seconds = duration.as_secs();
    let micros = duration.subsec_micros();
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("[{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{micros:06}Z]")
}

pub fn format_log_line(id: u64, body: &str) -> String {
    format!("{} id={id} {body}", format_timestamp(SystemTime::now()))
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    // Howard Hinnant's civil-from-days algorithm. Converts days since
    // 1970-01-01 to proleptic Gregorian UTC date without adding a timezone
    // dependency for the log hot path.
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

pub fn read_log(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(data) => Ok(data),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::TempDir;

    #[test]
    fn formats_timestamp_with_microsecond_precision() {
        let time = UNIX_EPOCH + Duration::new(1_704_067_200, 123_456_789);
        assert_eq!(format_timestamp(time), "[2024-01-01T00:00:00.123456Z]");
    }

    #[test]
    fn formats_log_line_with_id_and_body() {
        let line = format_log_line(7, "pending path=/data/file SETATTR mode=0600");
        assert!(line.starts_with('['));
        assert!(line.contains("] id=7 pending path=/data/file SETATTR mode=0600"));
    }

    #[test]
    fn writer_serializes_reset_append_and_remove() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("sandbox.log");
        let writer = LogWriter::new();
        let handle = writer.handle();

        handle.reset(&path).unwrap();
        handle.append(&path, "one").unwrap();
        handle.append(&path, "two").unwrap();
        assert_eq!(read_log(&path).unwrap(), "one\ntwo\n");
        handle.remove(&path).unwrap();
        assert!(!path.exists());
        writer.shutdown().unwrap();
    }

    #[test]
    fn writer_preserves_every_concurrent_append_once() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("sandbox.log");
        let writer = LogWriter::new();
        let handle = writer.handle();
        handle.reset(&path).unwrap();

        let mut threads = Vec::new();
        for thread_id in 0..8 {
            let handle = handle.clone();
            let path = path.clone();
            threads.push(std::thread::spawn(move || {
                for item in 0..50 {
                    handle
                        .append(&path, format!("thread={thread_id} item={item}"))
                        .unwrap();
                }
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }

        let data = read_log(&path).unwrap();
        let lines: Vec<_> = data.lines().collect();
        assert_eq!(lines.len(), 400);
        let unique: HashSet<_> = lines.iter().copied().collect();
        assert_eq!(unique.len(), 400);
        for line in lines {
            assert!(line.starts_with("thread="));
            assert!(line.contains(" item="));
        }
        writer.shutdown().unwrap();
    }
}
