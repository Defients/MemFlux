mod action;
mod cleaner;
mod guardian;
mod settings;
mod telemetry;

pub fn run() {
    if let Some(exit_code) = action::elevated_helper_exit_code() {
        std::process::exit(exit_code as i32);
    }
    app::run();
}

mod app;
