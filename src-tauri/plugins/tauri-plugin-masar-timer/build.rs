const COMMANDS: &[&str] = &["start_foreground_timer", "stop_foreground_timer"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
