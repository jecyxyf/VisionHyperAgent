use chrono::{Days, Local};
use std::{fs, path::Path, process::Command};
use vision_hyper_agent::foundation::{self, AppConfig, CONFIG, ConfigLoadStatus, LOGGER};
#[path = "foundation/support.rs"]
mod support;

const FAKE_KEY: &str = "unit-test-api-key";
const BROKEN: &[u8] = br#"{"agent":"unit-test-api-key"}"#;

// Each case uses the public singletons in a fresh executable/process, not a mock manager.
fn run_case(mode: &str, prepare: impl FnOnce(&Path), verify: impl FnOnce(&Path, &str)) {
    let temp = support::TestDir::new();
    let app_dir = temp.path().join("程序 空格");
    let cwd = temp.path().join("different-cwd");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    prepare(&app_dir);
    let target = app_dir.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    fs::copy(std::env::current_exe().unwrap(), &target).unwrap();
    let output = Command::new(target)
        .args(["--exact", "singleton_child", "--nocapture"])
        .env("VHA_FOUNDATION_CHILD", mode)
        .env("VHA_FOUNDATION_SANDBOX", &app_dir)
        .current_dir(&cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("file-only-marker"));
    assert!(!stdout.contains(FAKE_KEY));
    assert!(!cwd.join("config.json").exists());
    assert!(!cwd.join("logs").exists());
    let mut files: Vec<_> = fs::read_dir(app_dir.join("logs"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "log"))
        .collect();
    files.sort();
    let logs: String = files
        .iter()
        .map(|p| fs::read_to_string(p).unwrap())
        .collect();
    assert!(!logs.contains(FAKE_KEY));
    assert!(logs.contains("file-only-marker"));
    verify(&app_dir, &logs);
}

#[test]
fn global_objects_are_executable_relative_and_file_only() {
    let today = Local::now().date_naive();
    let expired = format!("{}.log", today.checked_sub_days(Days::new(90)).unwrap());
    let retained = format!("{}.log", today.checked_sub_days(Days::new(89)).unwrap());
    run_case(
        "normal",
        |dir| {
            fs::create_dir(dir.join("logs")).unwrap();
            fs::write(dir.join("logs").join(&expired), "expired\n").unwrap();
            fs::write(dir.join("logs").join(&retained), "retained\n").unwrap();
            fs::write(dir.join("logs/notes.txt"), "untouched").unwrap();
        },
        |dir, logs| {
            for level in ["DEBUG", "INFO", "WARNING", "ERROR"] {
                assert!(logs.contains(&format!("[{level}]")));
            }
            assert!(logs.contains("depoly/tests/foundation.rs:"));
            assert!(!dir.join("logs").join(&expired).exists());
            // If the test crosses midnight, the former boundary correctly expires too.
            if Local::now().date_naive() == today {
                assert!(dir.join("logs").join(&retained).exists());
            }
            assert_eq!(
                fs::read_to_string(dir.join("logs/notes.txt")).unwrap(),
                "untouched"
            );
            let stored: AppConfig =
                serde_json::from_slice(&fs::read(dir.join("config.json")).unwrap()).unwrap();
            assert_eq!(stored.agent.api_key, FAKE_KEY);
            assert_eq!(stored.agent.model, "test-model");
            assert_eq!(
                logs.lines().filter(|line| line.contains("worker-")).count(),
                32
            );
        },
    );
}

#[test]
fn corrupted_startup_config_is_backed_up_and_rebuilt() {
    run_case(
        "corrupt",
        |dir| {
            fs::write(dir.join("config.json"), BROKEN).unwrap();
            fs::write(dir.join("config.json.back"), "previous-backup").unwrap();
        },
        |dir, logs| {
            assert_eq!(fs::read(dir.join("config.json.back")).unwrap(), BROKEN);
            let actual: AppConfig =
                serde_json::from_slice(&fs::read(dir.join("config.json")).unwrap()).unwrap();
            assert_eq!(actual, AppConfig::default());
            assert!(logs.contains("[ERROR]"));
            assert!(logs.contains("config_manager.rs:"));
            assert!(logs.contains("JSON 行：1"));
            assert!(logs.contains("配置字段类型不匹配"));
        },
    );
}

#[test]
fn failed_backup_does_not_overwrite_corrupt_config() {
    run_case(
        "backup-blocked",
        |dir| {
            fs::write(dir.join("config.json"), BROKEN).unwrap();
            fs::create_dir(dir.join("config.json.back")).unwrap();
            fs::write(dir.join("config.json.back/keep"), "untouched").unwrap();
        },
        |dir, logs| {
            assert_eq!(fs::read(dir.join("config.json")).unwrap(), BROKEN);
            assert_eq!(
                fs::read_to_string(dir.join("config.json.back/keep")).unwrap(),
                "untouched"
            );
            assert!(logs.contains("[ERROR]"));
            assert!(logs.contains("config.json.back"));
        },
    );
}

#[test]
fn failed_save_keeps_memory_and_does_not_replace_a_directory() {
    run_case(
        "save-blocked",
        |_| {},
        |dir, logs| {
            assert_eq!(
                fs::read_to_string(dir.join("config.json/keep")).unwrap(),
                "untouched"
            );
            assert!(logs.contains("[ERROR]"));
            assert!(!fs::read_dir(dir).unwrap().any(|e| {
                e.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".config.json")
            }));
        },
    );
}

#[test]
fn singleton_child() {
    let Ok(mode) = std::env::var("VHA_FOUNDATION_CHILD") else {
        return;
    };
    let executable = std::env::current_exe().unwrap();
    let dir = executable.parent().unwrap();
    let expected =
        std::env::var_os("VHA_FOUNDATION_SANDBOX").expect("child must run in test sandbox");
    assert_eq!(
        dir.canonicalize().unwrap(),
        Path::new(&expected).canonicalize().unwrap()
    );
    let path = dir.join("config.json");
    match mode.as_str() {
        "normal" => {
            assert!(matches!(
                foundation::init().unwrap(),
                ConfigLoadStatus::Created
            ));
            CONFIG
                .update(|cfg| {
                    cfg.agent.base_url = "https://example.invalid/v1".into();
                    cfg.agent.api_key = FAKE_KEY.into();
                    cfg.agent.model = "test-model".into();
                })
                .unwrap();
            assert!(!fs::read_to_string(&path).unwrap().contains(FAKE_KEY));
            assert!(matches!(
                foundation::init().unwrap(),
                ConfigLoadStatus::AlreadyInitialized
            ));
            assert_eq!(CONFIG.snapshot().unwrap().agent.model, "test-model");
            CONFIG.save().unwrap();
            assert!(fs::read_to_string(&path).unwrap().contains(FAKE_KEY));
            LOGGER
                .debug("test", &format!("{:?}", CONFIG.snapshot().unwrap()))
                .unwrap();
            LOGGER.info("test", "file-only-marker").unwrap();
            LOGGER.warning("test", "file-only-marker").unwrap();
            LOGGER.error("test", "file-only-marker").unwrap();
            std::thread::scope(|scope| {
                for worker in 0..4 {
                    scope.spawn(move || {
                        for item in 0..8 {
                            CONFIG.update(|cfg| cfg.agent.model.push('x')).unwrap();
                            LOGGER
                                .info("parallel", &format!("worker-{worker}-{item}"))
                                .unwrap();
                        }
                    });
                }
            });
            assert_eq!(
                CONFIG.snapshot().unwrap().agent.model,
                format!("test-model{}", "x".repeat(32))
            );
            CONFIG.load().unwrap();
            assert_eq!(CONFIG.snapshot().unwrap().agent.model, "test-model");
        }
        "corrupt" => {
            let ConfigLoadStatus::Recovered(context) = foundation::init().unwrap() else {
                panic!("expected recovery")
            };
            LOGGER
                .error_with_context("config", "file-only-marker", &context)
                .unwrap();
            assert_eq!(CONFIG.snapshot().unwrap(), AppConfig::default());
        }
        "backup-blocked" => {
            let error = foundation::init().unwrap_err();
            LOGGER
                .error_with_context("config", "file-only-marker", error.context())
                .unwrap();
            assert!(CONFIG.snapshot().is_err());
        }
        "save-blocked" => {
            foundation::init().unwrap();
            CONFIG
                .update(|cfg| cfg.agent.model = "pending".into())
                .unwrap();
            fs::remove_file(&path).unwrap();
            fs::create_dir(&path).unwrap();
            fs::write(path.join("keep"), "untouched").unwrap();
            let error = CONFIG.save().unwrap_err();
            LOGGER
                .error_with_context("config", "file-only-marker", error.context())
                .unwrap();
            assert_eq!(CONFIG.snapshot().unwrap().agent.model, "pending");
        }
        _ => panic!("unknown test case"),
    }
    LOGGER.flush().unwrap();
}

#[test]
fn foundation_source_has_only_the_approved_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/foundation");
    let mut files: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files, ["config_manager.rs", "logger.rs", "mod.rs"]);
}
