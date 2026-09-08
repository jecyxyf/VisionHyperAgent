use super::*;
use crate::foundation::test_support::TestDir;
use std::sync::Arc;

#[test]
fn defaults_memory_only_save_load_and_idempotent_init() {
    let dir = TestDir::new();
    let path = dir.path().join("config.json");
    let manager = ConfigManager::new();
    assert_eq!(
        manager.init_file(path.clone()).unwrap(),
        ConfigLoadStatus::Created
    );
    let original = fs::read(&path).unwrap();
    assert_eq!(
        serde_json::from_slice::<AppConfig>(&original).unwrap(),
        AppConfig::default()
    );
    manager
        .update(|c| {
            c.agent.base_url = "https://example.invalid/v1".into();
            c.agent.api_key = "unit-test-api-key".into();
            c.agent.model = "模型-v1".into();
        })
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(
        manager.init_file(path.clone()).unwrap(),
        ConfigLoadStatus::AlreadyInitialized
    );
    assert_eq!(manager.snapshot().unwrap().agent.model, "模型-v1");
    assert!(!format!("{:?}", manager.snapshot().unwrap()).contains("unit-test-api-key"));
    manager.save().unwrap();
    let stored = fs::read(&path).unwrap();
    assert_eq!(stored.last(), Some(&b'\n'));
    let json: serde_json::Value = serde_json::from_slice(&stored).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 1);
    assert_eq!(json["agent"].as_object().unwrap().len(), 3);
    assert_eq!(json["agent"]["api_key"], "unit-test-api-key");
    manager
        .update(|c| c.agent.model = "unsaved".into())
        .unwrap();
    assert_eq!(manager.load().unwrap(), ConfigLoadStatus::Loaded);
    assert_eq!(manager.snapshot().unwrap().agent.model, "模型-v1");
}

#[test]
fn missing_known_fields_get_empty_defaults() {
    let dir = TestDir::new();
    let path = dir.path().join("config.json");
    fs::write(&path, r#"{"agent":{"model":"model-only"}}"#).unwrap();
    let manager = ConfigManager::new();
    manager.init_file(path.clone()).unwrap();
    let config = manager.snapshot().unwrap();
    assert_eq!(config.agent.model, "model-only");
    assert_eq!(config.agent.api_key, "");
    assert_eq!(config.agent.base_url, "");
    fs::write(&path, "{}").unwrap();
    manager.load().unwrap();
    assert_eq!(manager.snapshot().unwrap(), AppConfig::default());
}

#[test]
fn corrupted_type_and_syntax_backups_preserve_bytes_without_secret_diagnostics() {
    let dir = TestDir::new();
    let path = dir.path().join("config.json");
    let back = dir.path().join("config.json.back");
    let manager = ConfigManager::new();
    manager.init_file(path.clone()).unwrap();
    for broken in [
        b"{\n \"agent\":\n".as_slice(),
        br#"{"agent":"unit-test-api-key"}"#.as_slice(),
        b"\xff\xfe".as_slice(),
    ] {
        fs::write(&path, broken).unwrap();
        let status = manager.load().unwrap();
        assert!(!format!("{status:?}").contains("unit-test-api-key"));
        let ConfigLoadStatus::Recovered(context) = status else {
            panic!("expected recovery")
        };
        assert_eq!(context.data_file.as_deref(), Some(path.as_path()));
        assert!(context.data_line.is_some() && context.data_column.is_some());
        assert!(context.source.file.ends_with("config_manager.rs") && context.source.line > 0);
        assert_eq!(fs::read(&back).unwrap(), broken);
        assert_eq!(
            serde_json::from_slice::<AppConfig>(&fs::read(&path).unwrap()).unwrap(),
            AppConfig::default()
        );
    }
}

#[test]
fn backup_failure_preserves_original_and_memory() {
    let dir = TestDir::new();
    let path = dir.path().join("config.json");
    let manager = ConfigManager::new();
    manager.init_file(path.clone()).unwrap();
    manager
        .update(|c| c.agent.model = "pending".into())
        .unwrap();
    let broken = b"{broken";
    fs::write(&path, broken).unwrap();
    let backup = dir.path().join("config.json.back");
    fs::create_dir(&backup).unwrap();
    fs::write(backup.join("keep"), "safe").unwrap();
    let error = manager.load().unwrap_err();
    assert_eq!(error.kind(), FoundationErrorKind::Io);
    assert_eq!(fs::read(&path).unwrap(), broken);
    assert_eq!(manager.snapshot().unwrap().agent.model, "pending");
    assert_eq!(fs::read_to_string(backup.join("keep")).unwrap(), "safe");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn io_errors_are_not_recovered_as_bad_json() {
    let dir = TestDir::new();
    let path = dir.path().join("config.json");
    fs::create_dir(&path).unwrap();
    let manager = ConfigManager::new();
    let error = manager.init_file(path.clone()).unwrap_err();
    assert_eq!(error.kind(), FoundationErrorKind::Io);
    assert!(path.is_dir());
    assert!(!dir.path().join("config.json.back").exists());
    assert_eq!(
        manager.snapshot().unwrap_err().kind(),
        FoundationErrorKind::NotInitialized
    );
}

#[test]
fn failed_write_does_not_truncate_old_target_or_leave_temporary_file() {
    let dir = TestDir::new();
    let path = dir.path().join("config.json");
    fs::write(&path, "original").unwrap();
    let result = atomic_write_with(&path, |f| {
        f.write_all(b"partial")?;
        Err(io::Error::other("injected write failure"))
    });
    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "original");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn save_replacement_failure_preserves_memory_and_destination() {
    let dir = TestDir::new();
    let path = dir.path().join("config.json");
    let manager = ConfigManager::new();
    manager.init_file(path.clone()).unwrap();
    manager
        .update(|c| c.agent.model = "pending".into())
        .unwrap();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("keep"), "safe").unwrap();
    assert!(manager.save().is_err());
    assert_eq!(manager.snapshot().unwrap().agent.model, "pending");
    assert_eq!(fs::read_to_string(path.join("keep")).unwrap(), "safe");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn concurrent_updates_are_serialized_and_saves_are_valid_json() {
    let dir = TestDir::new();
    let path = dir.path().join("config.json");
    let manager = Arc::new(ConfigManager::new());
    manager.init_file(path.clone()).unwrap();
    std::thread::scope(|scope| {
        for _ in 0..6 {
            let m = Arc::clone(&manager);
            scope.spawn(move || {
                for _ in 0..15 {
                    m.update(|c| c.agent.model.push('x')).unwrap();
                    m.save().unwrap();
                }
            });
        }
    });
    manager.save().unwrap();
    let config = manager.snapshot().unwrap();
    assert_eq!(config.agent.model.len(), 90);
    assert_eq!(
        serde_json::from_slice::<AppConfig>(&fs::read(path).unwrap()).unwrap(),
        config
    );
}

#[test]
fn poison_and_uninitialized_access_return_errors() {
    let manager = ConfigManager::new();
    assert!(manager.snapshot().is_err());
    assert!(manager.save().is_err());
    assert!(manager.load().is_err());
    assert!(manager.update(|_| {}).is_err());
    let dir = TestDir::new();
    manager.init_file(dir.path().join("config.json")).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = manager.update(|_| panic!("test callback panic"));
    }));
    assert!(result.is_err());
    assert_eq!(
        manager.snapshot().unwrap_err().kind(),
        FoundationErrorKind::LockPoisoned
    );
    assert!(manager.save().is_err());
}

#[cfg(unix)]
#[test]
fn config_and_backups_have_private_permissions_and_accept_non_utf8_directories() {
    use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
    let dir = TestDir::new();
    let base = dir.path().join(OsString::from_vec(vec![b'd', 0xff]));
    fs::create_dir(&base).unwrap();
    let path = base.join("config.json");
    let manager = ConfigManager::new();
    manager.init_file(path.clone()).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::write(&path, b"{broken").unwrap();
    manager.load().unwrap();
    for file in [&path, &base.join("config.json.back")] {
        assert_eq!(
            fs::metadata(file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn rebuild_failure_keeps_completed_backup() {
    let dir = TestDir::new();
    let path = dir.path().join("config.json");
    let broken = b"{broken-original";
    fs::write(&path, broken).unwrap();
    let result = read_or_recover_with(&path, |destination, data| {
        if destination == path {
            Err(FoundationError::io(
                "injected replacement failure",
                Some(destination),
                io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
            ))
        } else {
            atomic_write(destination, data)
        }
    });
    assert!(result.is_err());
    assert_eq!(fs::read(&path).unwrap(), broken);
    assert_eq!(
        fs::read(dir.path().join("config.json.back")).unwrap(),
        broken
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}
