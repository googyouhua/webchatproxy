use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;

pub struct Logger {
    path: String,
}

impl Logger {
    pub fn new(path: &str) -> Self {
        Self { path: path.to_string() }
    }

    pub fn log(&mut self, message: &str) {
        let ts = Utc::now().format("%Y-%m-%dT%H:%M:%S%.9fZ");
        let line = format!("[{}] {}\n", ts, message);
        eprint!("{}", line);
        if let Ok(mut f) = OpenOptions::new()
            .create(true).append(true).open(&self.path)
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_log_writes_to_file() {
        let path = "/tmp/webai-proxy-test.log";
        let _ = fs::remove_file(path);

        let mut logger = Logger::new(path);
        logger.log("test message");

        let content = fs::read_to_string(path).unwrap_or_default();
        assert!(content.contains("test message"), "log should contain message");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_log_contains_timestamp() {
        let path = "/tmp/webai-proxy-test-ts.log";
        let _ = fs::remove_file(path);

        let mut logger = Logger::new(path);
        logger.log("hello");

        let content = fs::read_to_string(path).unwrap_or_default();
        assert!(content.starts_with('['), "log should start with timestamp bracket");
        assert!(content.contains("hello"), "log should contain message");
        let _ = fs::remove_file(path);
    }
}
