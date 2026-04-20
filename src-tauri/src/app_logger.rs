use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

static BUFFER: OnceLock<Arc<Mutex<Vec<String>>>> = OnceLock::new();

/// Install a log::Log backend that pushes every record into the shared app_logs
/// buffer (visible in the UI Logs → App tab) and also mirrors to stderr.
pub fn init(buffer: Arc<Mutex<Vec<String>>>) {
    let _ = BUFFER.set(buffer);
    let _ = log::set_logger(&AppLogger);
    log::set_max_level(log::LevelFilter::Info);
}

struct AppLogger;

impl log::Log for AppLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let ts = chrono_now();
        let line = format!("[{}] {:<5} {} - {}", ts, record.level(), record.target(), record.args());
        eprintln!("{}", line);

        // Synchronous try_lock — never blocks, never panics, never needs tokio runtime.
        // If another task is holding the mutex we drop this line; acceptable for a log.
        if let Some(buf) = BUFFER.get() {
            if let Ok(mut g) = buf.try_lock() {
                g.push(line);
                let len = g.len();
                if len > 2000 {
                    g.drain(0..len - 1500);
                }
            }
        }
    }

    fn flush(&self) {}
}

fn chrono_now() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = t.as_secs();
    let ms = t.subsec_millis();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, ms)
}
