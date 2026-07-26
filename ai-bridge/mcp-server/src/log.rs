use std::fs::OpenOptions;
use std::io::Write;

pub struct Logger {
    file: std::fs::File,
}

impl Logger {
    pub fn new(path: &str) -> Self {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("Failed to open log file");
        Logger { file }
    }

    pub fn log(&mut self, msg: &str) {
        let line = format!("[{}] {}", chrono::Utc::now().to_rfc3339(), msg);
        eprintln!("{}", line);
        let _ = writeln!(self.file, "{}", line);
    }
}
