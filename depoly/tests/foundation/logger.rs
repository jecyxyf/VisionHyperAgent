use super::*;
use crate::foundation::test_support::TestDir;
use std::{collections::HashSet, sync::Arc};
fn time(s: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(s).unwrap()
}

#[test]
fn timestamps_levels_source_and_control_characters() {
    let dir = TestDir::new();
    let logger = Logger::new();
    let now = time("2026-09-08T13:14:15.123+08:00");
    logger
        .init_directory(dir.path().to_owned(), now.date_naive())
        .unwrap();
    for level in [
        LogLevel::Debug,
        LogLevel::Info,
        LogLevel::Warning,
        LogLevel::Error,
    ] {
        logger
            .record(
                level,
                "module\ninjection",
                "reason\nnext\rline\tend",
                SourceLocation {
                    file: "src/agent/client.rs",
                    line: 42,
                },
                now,
            )
            .unwrap();
    }
    logger.flush().unwrap();
    let text = fs::read_to_string(dir.path().join("2026-09-08.log")).unwrap();
    assert_eq!(text.lines().count(), 4);
    for level in ["DEBUG", "INFO", "WARNING", "ERROR"] {
        assert!(text.contains(&format!("[{level}]")));
    }
    assert!(text.contains("2026-09-08 13:14:15.123"));
    assert!(text.contains("[src/agent/client.rs:42]"));
    assert!(text.contains("module\\ninjection"));
    assert!(text.contains("reason\\nnext\\rline\\tend"));
}

#[test]
fn caller_and_error_origin_are_not_the_logger_implementation() {
    let dir = TestDir::new();
    let logger = Logger::new();
    logger
        .init_directory(dir.path().to_owned(), Local::now().date_naive())
        .unwrap();
    let caller_line = line!() + 1;
    logger.error("test", "direct error").unwrap();
    let origin_line = line!() + 1;
    let context = ErrorContext::new("outer reason")
        .with_cause("inner reason")
        .with_data_file("config.json")
        .with_json_position(8, 17);
    logger
        .error_with_context("config", "configuration error", &context)
        .unwrap();
    let logs: String = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| fs::read_to_string(e.unwrap().path()).unwrap())
        .collect();
    assert!(logs.contains(&format!("[{}:{caller_line}]", file!())));
    assert!(logs.contains(&format!("[{}:{origin_line}]", file!())));
    assert!(
        logs.contains("outer reason；原因：inner reason；文件：config.json；JSON 行：8，列：17")
    );
}

#[test]
fn append_restart_daily_rotation_and_calendar_retention() {
    let dir = TestDir::new();
    let today = time("2026-09-08T23:59:59+08:00");
    let day = today.date_naive();
    let keep = day.checked_sub_days(Days::new(89)).unwrap();
    let old = day.checked_sub_days(Days::new(90)).unwrap();
    let future = day.checked_add_days(Days::new(30)).unwrap();
    for d in [keep, old, future] {
        fs::write(dir.path().join(log_name(d)), "existing\n").unwrap();
    }
    fs::write(dir.path().join("notes.log"), "untouched").unwrap();
    fs::write(dir.path().join("2026-01-01.log.back"), "untouched").unwrap();
    fs::create_dir(dir.path().join("2025-01-01.log")).unwrap();
    let logger = Logger::new();
    logger.init_directory(dir.path().to_owned(), day).unwrap();
    assert!(!dir.path().join(log_name(old)).exists());
    assert!(dir.path().join(log_name(keep)).exists());
    assert!(dir.path().join(log_name(future)).exists());
    assert!(dir.path().join("notes.log").exists());
    assert!(dir.path().join("2025-01-01.log").is_dir());
    logger
        .record(
            LogLevel::Info,
            "test",
            "first",
            SourceLocation::caller(),
            today,
        )
        .unwrap();
    drop(logger);
    let logger = Logger::new();
    logger.init_directory(dir.path().to_owned(), day).unwrap();
    logger
        .record(
            LogLevel::Info,
            "test",
            "second",
            SourceLocation::caller(),
            today,
        )
        .unwrap();
    let tomorrow = time("2026-09-09T00:00:01+08:00");
    logger
        .record(
            LogLevel::Info,
            "test",
            "third",
            SourceLocation::caller(),
            tomorrow,
        )
        .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("2026-09-08.log"))
            .unwrap()
            .lines()
            .count(),
        2
    );
    assert!(
        fs::read_to_string(dir.path().join("2026-09-09.log"))
            .unwrap()
            .contains("third")
    );
    assert!(!dir.path().join(log_name(keep)).exists());
}

#[test]
fn initialization_errors_and_repeated_initialization() {
    let logger = Logger::new();
    let dir = TestDir::new();
    assert_eq!(
        logger.info("test", "x").unwrap_err().kind(),
        FoundationErrorKind::NotInitialized
    );
    assert_eq!(
        logger.flush().unwrap_err().kind(),
        FoundationErrorKind::NotInitialized
    );
    let blocked = dir.path().join("not-a-directory");
    fs::write(&blocked, "keep").unwrap();
    assert!(
        logger
            .init_directory(blocked.clone(), Local::now().date_naive())
            .is_err()
    );
    assert_eq!(fs::read_to_string(blocked).unwrap(), "keep");
    logger
        .init_directory(dir.path().to_owned(), Local::now().date_naive())
        .unwrap();
    logger.info("test", "one").unwrap();
    logger
        .init_directory(dir.path().to_owned(), Local::now().date_naive())
        .unwrap();
    logger.info("test", "two").unwrap();
    let text = fs::read_to_string(dir.path().join(log_name(Local::now().date_naive()))).unwrap();
    assert_eq!(text.lines().count(), 2);
}

#[test]
fn thread_writes_are_whole_unique_records() {
    let dir = TestDir::new();
    let logger = Arc::new(Logger::new());
    logger
        .init_directory(dir.path().to_owned(), Local::now().date_naive())
        .unwrap();
    std::thread::scope(|scope| {
        for worker in 0..8 {
            let logger = Arc::clone(&logger);
            scope.spawn(move || {
                for item in 0..50 {
                    logger
                        .info("parallel", &format!("record-{worker}-{item}"))
                        .unwrap();
                }
            });
        }
    });
    let text: String = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| fs::read_to_string(e.unwrap().path()).unwrap())
        .collect();
    let lines: HashSet<_> = text.lines().collect();
    assert_eq!(lines.len(), 400);
    for line in lines {
        assert_eq!(line.matches("[INFO]").count(), 1);
        assert_eq!(line.matches("record-").count(), 1);
    }
}

#[test]
fn log_failure_does_not_fall_back_or_hide_errors() {
    let dir = TestDir::new();
    let logger = Logger::new();
    let day = time("2026-09-08T12:00:00+08:00");
    logger
        .init_directory(dir.path().to_owned(), day.date_naive())
        .unwrap();
    fs::create_dir(dir.path().join("2026-09-09.log")).unwrap();
    assert!(
        logger
            .record(
                LogLevel::Info,
                "test",
                "cannot write",
                SourceLocation::caller(),
                time("2026-09-09T01:00:00+08:00")
            )
            .is_err()
    );
    assert!(dir.path().join("2026-09-09.log").is_dir());
}

#[cfg(unix)]
#[test]
fn retention_does_not_follow_symlinks() {
    use std::os::unix::fs::symlink;
    let dir = TestDir::new();
    let outside = TestDir::new();
    let target = outside.path().join("keep");
    fs::write(&target, "safe").unwrap();
    let link = dir.path().join("2000-01-01.log");
    symlink(&target, &link).unwrap();
    clean_expired(dir.path(), time("2026-09-08T00:00:00Z").date_naive()).unwrap();
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_to_string(target).unwrap(), "safe");
}

#[test]
fn retention_read_failure_is_returned() {
    let dir = TestDir::new();
    let not_a_directory = dir.path().join("file");
    fs::write(&not_a_directory, "keep").unwrap();
    let error =
        clean_expired(&not_a_directory, time("2026-09-08T00:00:00Z").date_naive()).unwrap_err();
    assert_eq!(error.kind(), FoundationErrorKind::Io);
    assert_eq!(fs::read_to_string(not_a_directory).unwrap(), "keep");
}
