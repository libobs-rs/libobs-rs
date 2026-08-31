use std::{
    fs::{File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::Path,
};

use fs2::FileExt;

/// Guard for the cross-process OBS cache lock.
///
/// The lock file intentionally remains on disk after this guard is dropped.
/// Mutual exclusion is provided by the OS file lock held by `_file`, not by
/// the presence/absence of the pathname. Leaving the file in place avoids a
/// race where one waiter removes a lock file while another process already has
/// that same file open and locked.
pub struct LockGuard {
    _file: File,
}

/// Acquires the OBS cache lock, blocking until any other process using the same
/// cache has released it.
///
/// OS-backed file locks are automatically released if the process exits or is
/// killed, so an OOM/crash cannot leave a permanently-live PID lock behind.
pub fn acquire_lock(lock: &Path) -> anyhow::Result<LockGuard> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock)?;
    file.lock_exclusive()?;

    // Keep a PID in the file for diagnostics only. Correctness relies on the
    // OS lock above, not on this content.
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    writeln!(file, "{}", std::process::id())?;
    file.flush()?;

    Ok(LockGuard { _file: file })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::mpsc,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    fn unique_lock_path(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "cargo-obs-build-{label}-{}-{nonce}.lock",
            std::process::id()
        ))
    }

    #[test]
    fn lock_can_be_reacquired_after_guard_drop() {
        let path = unique_lock_path("reacquire");
        drop(acquire_lock(&path).unwrap());
        drop(acquire_lock(&path).unwrap());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn second_acquirer_waits_for_first_guard() {
        let path = unique_lock_path("blocking");
        let first = acquire_lock(&path).unwrap();
        let worker_path = path.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let second = acquire_lock(&worker_path).unwrap();
            sender.send(()).unwrap();
            drop(second);
        });

        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        drop(first);
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
