#![windows_subsystem = "windows"]

#[cfg(not(windows))]
compile_error!("NetMap requires a Windows MSVC target");

mod host;

fn main() {
    if let Err(error) = host::run() {
        host::show_startup_error(&error);
        std::process::exit(1);
    }
}
