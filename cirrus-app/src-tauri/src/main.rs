#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

fn main() {
  tauri::Builder::default()
    .plugin(cirrus_lib::tauri_plugin::init())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}