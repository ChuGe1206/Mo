#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod windows_app;

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    match windows_app::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            windows_app::show_fatal_error(&error);
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(windows))]
fn main() -> std::process::ExitCode {
    eprintln!("mo-settings currently supports Windows only");
    std::process::ExitCode::FAILURE
}
