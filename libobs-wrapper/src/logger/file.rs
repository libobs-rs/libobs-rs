use std::{fs::File, path::Path};

use chrono::Local;

use super::ObsLogger;
use crate::utils::ObsError;

/// A logger that writes logs to a file
#[derive(Debug)]
pub struct FileLogger {
    file: File,
}

impl FileLogger {
    /// Creates a new `FileLogger`, which writes to a log file formatted by the current time.
    /// This does not implement any rotary logging or similar, so there'll be a log file for every time your ObsContext is being started up.
    pub fn from_dir(dir: &Path) -> Result<Self, ObsError> {
        let current_local = Local::now();
        let custom_format = current_local.format("%Y-%m-%d-%H-%M-%S");

        Ok(Self {
            file: File::create(dir.join(format!("obs-{}.log", custom_format)))
                .map_err(|e| ObsError::IoError(e.to_string()))?,
        })
    }

    /// Creates a new `FileLogger` which will pipe the libobs output directly to the file given.
    pub fn from_file(file: &Path) -> Result<Self, ObsError> {
        Ok(Self {
            file: File::create(file).map_err(|e| ObsError::IoError(e.to_string()))?,
        })
    }
}

impl ObsLogger for FileLogger {
    fn log(&mut self, level: crate::enums::ObsLogLevel, msg: String) {
        use std::io::Write;
        if let Err(err) = writeln!(self.file, "[{:?}] {}", level, msg) {
            eprintln!("Failed to write libobs log message: {err}");
        }
    }
}
