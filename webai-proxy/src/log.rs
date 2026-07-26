use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::sync::OnceLock;

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

pub fn default_log_path() -> String {
    let mut p = std::env::temp_dir();
    p.push("webai-proxy.log");
    p.to_string_lossy().to_string()
}

static GLOBAL_LOGGER: OnceLock<Mutex<Logger>> = OnceLock::new();

pub fn init_global_logger(path: &str) {
    GLOBAL_LOGGER.get_or_init(|| Mutex::new(Logger::new(path)));
}

pub fn global_log(message: &str) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        logger.lock().unwrap().log(message);
    } else {
        eprintln!("{}", message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_log_path(name: &str) -> String {
        let mut p = std::env::temp_dir();
        p.push(name);
        p.to_string_lossy().to_string()
    }

    #[test]
    fn test_default_log_path_uses_temp_dir() {
        let path = super::default_log_path();
        let expected_prefix = std::env::temp_dir().to_string_lossy().to_string();
        assert!(path.starts_with(&expected_prefix), "should use temp dir");
        assert!(path.contains("webai-proxy.log"), "should contain filename");
    }

    #[test]
    fn test_log_writes_to_file() {
        let path = temp_log_path("webai-proxy-test.log");
        let _ = fs::remove_file(&path);

        let mut logger = Logger::new(&path);
        logger.log("test message");

        let content = fs::read_to_string(&path).unwrap_or_default();
        assert!(content.contains("test message"), "log should contain message");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_log_contains_timestamp() {
        let path = temp_log_path("webai-proxy-test-ts.log");
        let _ = fs::remove_file(&path);

        let mut logger = Logger::new(&path);
        logger.log("hello");

        let content = fs::read_to_string(&path).unwrap_or_default();
        assert!(content.starts_with('['), "log should start with timestamp bracket");
        assert!(content.contains("hello"), "log should contain message");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_global_log() {
        let path = temp_log_path("webai-proxy-global-test.log");
        let _ = fs::remove_file(&path);

        init_global_logger(&path);
        global_log("first write");
        global_log("second write");

        let content = fs::read_to_string(&path).unwrap_or_default();
        assert!(content.contains("first write"), "should contain first message");
        assert!(content.contains("second write"), "should contain second message");
        let _ = fs::remove_file(&path);
    }
}
