#[cfg(windows)]
mod windows;

#[cfg(windows)]
fn main() {
    windows::main().unwrap();
}

#[cfg(not(windows))]
fn main() {
    println!(
        "This example only supports windows at the moment, but you can implement similar code on Linux."
    );
}
