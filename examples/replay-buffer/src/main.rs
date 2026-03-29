#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(windows)]
fn main() {
    windows::main().unwrap();
}

#[cfg(target_os = "linux")]
fn main() {
    linux::main().unwrap();
}
