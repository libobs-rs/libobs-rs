#[cfg(target_os = "linux")]
mod linux;

fn main() {
    #[cfg(target_os = "linux")]
    linux::main().unwrap();

    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("This example is only supported on Linux.");
    }
}
