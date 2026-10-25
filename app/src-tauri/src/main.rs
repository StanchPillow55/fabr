#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(code) = fabr_core::remote::askpass::run_helper_if_invoked() {
        std::process::exit(code);
    }
    fabr_desktop::run();
}
