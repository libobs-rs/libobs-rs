use arboard::Clipboard;
use dialog::{Choice, DialogBox};

use super::ObsCrashHandler;

pub struct DialogCrashHandler {
    _private: (),
}

impl Default for DialogCrashHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl DialogCrashHandler {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl ObsCrashHandler for DialogCrashHandler {
    fn handle_crash(&self, message: String) {
        eprintln!("OBS crashed: {}", message);
        let res =
            dialog::Question::new("OBS has crashed. Do you want to copy the error to clipboard?")
                .title("OBS Crash Handler")
                .show();

        let res = match res {
            Ok(choice) => choice,
            Err(e) => {
                eprintln!("Failed to show crash handler dialog: {e:?}");
                return;
            }
        };
        if res == Choice::No {
            return;
        }

        let clipboard = Clipboard::new();
        let mut clipboard = match clipboard {
            Ok(clipboard) => clipboard,
            Err(e) => {
                eprintln!("Failed to create clipboard: {e:?}");
                return;
            }
        };
        if let Err(e) = clipboard.set_text(message.clone()) {
            eprintln!("Failed to copy crash message to clipboard: {e:?}");
        }
    }
}
