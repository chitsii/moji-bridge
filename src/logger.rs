use std::fs::OpenOptions;
use std::io::Write;

pub fn log(message: &str) {
    let log_path = std::env::temp_dir().join("moji-bridge-debug.log");
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = writeln!(file, "{}", message);
    }
}
