use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_MUTEX: Mutex<()> = Mutex::new(());

/// Formats current SystemTime as UTC "YYYY-MM-DD HH:MM:SS" without external dependencies
pub fn format_current_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs();
    let sec = (total_secs % 60) as u32;
    let min = ((total_secs / 60) % 60) as u32;
    let hour = ((total_secs / 3600) % 24) as u32;
    let days = (total_secs / 86400) as i64;

    // Convert days since 1970-01-01 to year, month, day (Howard Hinnant civil algorithm)
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC", y, m, d, hour, min, sec)
}

/// Initializes the global log file path in base_dir/launcher_log.txt and writes startup banner
pub fn init_logger(base_dir: &Path) {
    let path = base_dir.join("launcher_log.txt");
    let _ = LOG_PATH.set(path.clone());

    // Write session divider banner
    let sep = "================================================================================";
    write_log("INFO", sep);
    write_log(
        "INFO",
        &format!(
            "God of War III Launcher Log started (v1.8.1) | OS: {} | Arch: {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    );
    write_log("INFO", &format!("Base Directory: {}", base_dir.display()));
    write_log("INFO", sep);
}

/// Appends a formatted log line to stdout/stderr and launcher_log.txt
pub fn write_log(level: &str, message: &str) {
    let timestamp = format_current_timestamp();
    let formatted = format!("[{}] [{}] {}", timestamp, level, message);

    if level == "ERROR" {
        eprintln!("{}", formatted);
    } else {
        println!("{}", formatted);
    }

    if let Some(path) = LOG_PATH.get() {
        if let Ok(_guard) = LOG_MUTEX.lock() {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{}", formatted);
            }
        }
    }
}

/// Reads the current content of launcher_log.txt
pub fn read_launcher_log(base_dir: &Path) -> String {
    let path = base_dir.join("launcher_log.txt");
    if path.is_file() {
        fs::read_to_string(&path).unwrap_or_else(|e| format!("Log dosyası okunamadı: {}", e))
    } else {
        "Launcher log dosyası henüz oluşturulmadı.".to_string()
    }
}

/// Opens any file in the OS default text editor or viewer (e.g. Notepad, xdg-open)
pub fn open_file_path(path: &Path) {
    if !path.exists() {
        return;
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy()])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(path)
            .spawn();
    }
}

/// Opens the launcher_log.txt file in the OS default text editor (e.g. Notepad, xdg-open)
pub fn open_log_in_editor(base_dir: &Path) {
    let log_file = base_dir.join("launcher_log.txt");
    if !log_file.exists() {
        let _ = fs::write(&log_file, "God of War III Launcher Log\n");
    }
    open_file_path(&log_file);
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logger::write_log("INFO", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::logger::write_log("WARN", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::logger::write_log("ERROR", &format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_current_timestamp() {
        let ts = format_current_timestamp();
        assert!(ts.contains("UTC"));
        assert_eq!(ts.len(), 23); // "YYYY-MM-DD HH:MM:SS UTC"
    }

    #[test]
    fn test_write_and_read_log() {
        let temp_dir = std::env::temp_dir().join("gow3_logger_test");
        let _ = fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("launcher_log.txt");
        let _ = fs::remove_file(&test_file);

        init_logger(&temp_dir);
        log_info!("Test info message 123");
        log_warn!("Test warning message 456");
        log_error!("Test error message 789");

        let content = read_launcher_log(&temp_dir);
        assert!(content.contains("Test info message 123"));
        assert!(content.contains("Test warning message 456"));
        assert!(content.contains("Test error message 789"));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
