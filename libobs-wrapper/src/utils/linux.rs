#[derive(Debug)]
pub struct LinuxGlibLoop {
    glib_loop: glib::MainLoop,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl LinuxGlibLoop {
    pub fn new() -> Self {
        let g_loop = glib::MainLoop::new(None, false);
        let g_loop_clone = g_loop.clone();
        let handle = std::thread::spawn(move || {
            g_loop_clone.run();
        });

        Self {
            glib_loop: g_loop,
            handle: Some(handle),
        }
    }
}

impl Default for LinuxGlibLoop {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LinuxGlibLoop {
    fn drop(&mut self) {
        if self.glib_loop.is_running() {
            self.glib_loop.quit();
        }
        let handle = self.handle.take().unwrap();
        let r = handle.join();
        if std::thread::panicking() {
            log::error!(
                "[libobs-wrapper]: Thread panicked while dropping LinuxGlibLoop: {:?}",
                r.err()
            );
        } else {
            r.unwrap();
        }
    }
}
