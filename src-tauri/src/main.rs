// Prevents an additional console window on Windows in release builds.
// RecallPet only ships for macOS in Phase 0, but this stays harmless.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    recallpet_lib::run();
}
