// Release builds are GUI-subsystem so double-clicking the app never flashes a
// console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    sopkb_desktop_tauri_lib::run();
}
