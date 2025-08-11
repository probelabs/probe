use anyhow::{anyhow, Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process;
use tracing::{debug, info, warn};

/// PID file lock to ensure only one daemon instance runs at a time
pub struct PidLock {
    path: PathBuf,
    locked: bool,
}

impl PidLock {
    /// Create a new PID lock at the specified path
    pub fn new(socket_path: &str) -> Self {
        // Create PID file path based on socket path
        let pid_path = format!("{socket_path}.pid");
        Self {
            path: PathBuf::from(pid_path),
            locked: false,
        }
    }

    /// Try to acquire the lock, returning Ok if successful
    pub fn try_lock(&mut self) -> Result<()> {
        // Check if PID file exists
        if self.path.exists() {
            // Read the PID from the file
            let mut file = File::open(&self.path).context("Failed to open existing PID file")?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .context("Failed to read PID file")?;

            let pid: u32 = contents
                .trim()
                .parse()
                .context("Invalid PID in lock file")?;

            // Check if process is still running
            if is_process_running(pid) {
                return Err(anyhow!(
                    "Another daemon instance is already running (PID: {})",
                    pid
                ));
            } else {
                warn!(
                    "Found stale PID file for non-running process {}, removing",
                    pid
                );
                fs::remove_file(&self.path).context("Failed to remove stale PID file")?;
            }
        }

        // Write our PID to the file
        let pid = process::id();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)
            .context("Failed to create PID file")?;

        write!(file, "{pid}").context("Failed to write PID to lock file")?;

        self.locked = true;
        info!("Acquired PID lock at {:?} (PID: {})", self.path, pid);
        Ok(())
    }

    /// Release the lock by removing the PID file
    pub fn unlock(&mut self) -> Result<()> {
        if !self.locked {
            return Ok(());
        }

        if self.path.exists() {
            // Verify it's our PID before removing
            let mut file = File::open(&self.path)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;

            let pid: u32 = contents.trim().parse().unwrap_or(0);

            if pid == process::id() {
                fs::remove_file(&self.path).context("Failed to remove PID file")?;
                debug!("Released PID lock at {:?}", self.path);
            } else {
                warn!("PID file contains different PID ({}), not removing", pid);
            }
        }

        self.locked = false;
        Ok(())
    }
}

impl Drop for PidLock {
    fn drop(&mut self) {
        if self.locked {
            if let Err(e) = self.unlock() {
                warn!("Failed to unlock PID file on drop: {}", e);
            }
        }
    }
}

/// Check if a process with the given PID is running
#[cfg(unix)]
fn is_process_running(pid: u32) -> bool {
    // On Unix, we can check if a process exists by sending signal 0
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(windows)]
fn is_process_running(pid: u32) -> bool {
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            false
        } else {
            CloseHandle(handle);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_pid_lock_exclusive() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock").to_str().unwrap().to_string();

        let mut lock1 = PidLock::new(&socket_path);
        assert!(lock1.try_lock().is_ok(), "First lock should succeed");

        let mut lock2 = PidLock::new(&socket_path);
        assert!(lock2.try_lock().is_err(), "Second lock should fail");

        lock1.unlock().unwrap();
        assert!(lock2.try_lock().is_ok(), "Lock should succeed after unlock");
    }

    #[test]
    fn test_stale_pid_cleanup() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock").to_str().unwrap().to_string();
        let pid_path = format!("{socket_path}.pid");

        // Write a non-existent PID to the file
        std::fs::write(&pid_path, "99999999").unwrap();

        let mut lock = PidLock::new(&socket_path);
        assert!(
            lock.try_lock().is_ok(),
            "Should acquire lock after cleaning stale PID"
        );
        assert_eq!(
            std::fs::read_to_string(&pid_path).unwrap().trim(),
            process::id().to_string(),
            "PID file should contain current process ID"
        );
    }
}
