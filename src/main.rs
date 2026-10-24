#![windows_subsystem = "windows"]

#[cfg(not(windows))]
compile_error!("NetMap requires a Windows MSVC target");

mod app;
mod connections;
mod geography;
mod geolocation;
mod host;
mod map;

fn main() {
    if let Err(error) = app::run() {
        eprintln!("NetMap startup failed: {error}");
        // Windows may launch the saver unattended. Failure must never leave a
        // modal error over the desktop or settings preview.
        let unattended = std::env::args_os().skip(1).any(|arg| {
            let arg = arg.to_string_lossy().to_ascii_lowercase();
            arg == "/s"
                || arg == "-s"
                || arg.starts_with("/p")
                || arg.starts_with("-p")
                || arg == "--collector"
        });
        if !unattended {
            host::show_startup_error(&error);
        }
        std::process::exit(1);
    }
}
