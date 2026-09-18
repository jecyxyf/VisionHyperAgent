//! Generic JSON configuration storage with atomic writes and safe recovery.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig<T> {
    pub value: T,
    pub created: bool,
    pub recovered_from: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    Read,
    Backup,
    Write,
    Replace,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Read => "无法读取配置文件",
            Self::Backup => "无法备份损坏的配置文件",
            Self::Write => "无法写入配置临时文件",
            Self::Replace => "无法替换配置文件",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ConfigError {}

pub fn load_or_create<T>(path: &Path) -> Result<LoadedConfig<T>, ConfigError>
where
    T: Default + Serialize + DeserializeOwned,
{
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| ConfigError::Write)?;
    }
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let value = T::default();
            save(path, &value)?;
            return Ok(LoadedConfig {
                value,
                created: true,
                recovered_from: None,
            });
        }
        Err(_) => return Err(ConfigError::Read),
    };

    match serde_json::from_slice(&bytes) {
        Ok(value) => Ok(LoadedConfig {
            value,
            created: false,
            recovered_from: None,
        }),
        Err(_) => {
            let backup = backup_path(path);
            std::fs::copy(path, &backup).map_err(|_| ConfigError::Backup)?;
            let value = T::default();
            save(path, &value)?;
            Ok(LoadedConfig {
                value,
                created: false,
                recovered_from: Some(backup),
            })
        }
    }
}

pub fn save<T>(path: &Path, value: &T) -> Result<(), ConfigError>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| ConfigError::Write)?;
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.json");
    let mut temporary = tempfile::Builder::new()
        .prefix(&format!(".{file_name}."))
        .suffix(".tmp")
        .tempfile_in(directory)
        .map_err(|_| ConfigError::Write)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file_mut()
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|_| ConfigError::Write)?;
    }
    temporary
        .as_file_mut()
        .write_all(&bytes)
        .and_then(|_| temporary.as_file_mut().sync_all())
        .map_err(|_| ConfigError::Write)?;
    temporary.persist(path).map_err(|_| ConfigError::Replace)?;
    Ok(())
}

fn backup_path(path: &Path) -> PathBuf {
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%3fZ");
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("app_config.json");
    let mut backup = path.with_file_name(format!("{file_name}.invalid-{timestamp}"));
    let mut suffix = 1;
    while backup.exists() {
        backup = path.with_file_name(format!("{file_name}.invalid-{timestamp}-{suffix}"));
        suffix += 1;
    }
    backup
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
    struct Fixture {
        value: u32,
    }

    #[test]
    fn creates_a_private_default_and_rewrites_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested/app_config.json");
        let loaded = load_or_create::<Fixture>(&path).unwrap();
        assert!(loaded.created);
        assert_eq!(loaded.value, Fixture::default());
        assert!(path.is_file());

        save(&path, &Fixture { value: 7 }).unwrap();
        assert_eq!(load_or_create::<Fixture>(&path).unwrap().value.value, 7);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn backs_up_corrupt_json_and_rebuilds_default_without_losing_original() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("app_config.json");
        std::fs::write(&path, b"{\"secret-like-content\":").unwrap();
        let loaded = load_or_create::<Fixture>(&path).unwrap();
        assert!(!loaded.created);
        let backup = loaded.recovered_from.unwrap();
        assert_eq!(
            std::fs::read(&backup).unwrap(),
            b"{\"secret-like-content\":"
        );
        assert_eq!(loaded.value, Fixture::default());
        assert!(path.is_file());
    }
}
