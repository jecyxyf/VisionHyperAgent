//! Resolve the native executable, not npm's Node wrapper (which breaks parent-death ownership).
use std::io::Read;
use std::path::{Path, PathBuf};

pub const CODEX_PROCESS_FILE_NAME: &str = if cfg!(windows) {
    "VisionHyperAgentCodex.exe"
} else {
    "VisionHyperAgentCodex"
};

pub fn resolve_codex(configured: Option<&Path>, app_dir: &Path) -> Result<PathBuf, String> {
    if let Some(path) = configured {
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            app_dir.join(path)
        };
        return resolve_native(&path)
            .ok_or_else(|| "配置的 Codex 原生可执行文件不存在或无法识别".into());
    }
    let name = if cfg!(windows) { "codex.exe" } else { "codex" };
    for candidate in [app_dir.join(name), app_dir.join("depends/codex").join(name)] {
        if let Some(path) = resolve_native(&candidate) {
            return Ok(path);
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for name in [name, "codex.cmd", "codex.js"] {
                if let Some(path) = resolve_native(&dir.join(name)) {
                    return Ok(path);
                }
            }
        }
    }
    Err("未找到 Codex。请安装 Codex 或配置 codex.executable 指向原生可执行文件".into())
}

/// Install a private, recognizable copy of the native Codex runtime.
///
/// Windows derives the process image name from the executable file name. Copying the resolved
/// native binary once makes the owned child appear as VisionHyperAgentCodex.exe instead of reusing
/// the user's codex.exe image name. The copy also keeps this application independent from package
/// managers that may replace the source while a child is running.
pub fn install_process_alias(source: &Path, app_dir: &Path) -> Result<PathBuf, String> {
    let source = source
        .canonicalize()
        .map_err(|_| "无法访问配置的 Codex 原生可执行文件".to_string())?;
    let app_dir = app_dir
        .canonicalize()
        .map_err(|_| "无法访问 VisionHyperAgent 应用目录".to_string())?;
    let alias = app_dir.join(CODEX_PROCESS_FILE_NAME);

    if let Ok(existing) = alias.canonicalize() {
        if existing == source || is_native(&existing) {
            return Ok(existing);
        }
    }

    let temporary = app_dir.join(format!(
        ".{}.{}.tmp",
        CODEX_PROCESS_FILE_NAME,
        uuid::Uuid::new_v4().simple()
    ));
    let install = || -> std::io::Result<()> {
        std::fs::copy(&source, &temporary)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o755))?;
        }
        if alias.exists() && !is_native(&alias) {
            std::fs::remove_file(&alias)?;
        }
        std::fs::rename(&temporary, &alias)
    };
    if let Err(error) = install() {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!(
            "无法生成独立 Codex 进程文件 {}：{error}",
            alias.display()
        ));
    }

    alias
        .canonicalize()
        .map_err(|_| "无法确认独立 Codex 进程文件".to_string())
}

fn resolve_native(entry: &Path) -> Option<PathBuf> {
    let path = entry.canonicalize().ok()?;
    if is_native(&path) {
        return Some(path);
    }
    let (package, target, name) = platform()?;
    // Covers npm's optional platform package, older bundled vendor layouts, and Windows shims.
    for root in path.ancestors().skip(1).take(5) {
        let vendors = [
            root.join("vendor"),
            root.join("node_modules/@openai")
                .join(package)
                .join("vendor"),
            root.join("node_modules/@openai/codex/node_modules/@openai")
                .join(package)
                .join("vendor"),
            root.join("@openai").join(package).join("vendor"),
        ];
        for vendor in vendors {
            for directory in ["bin", "codex"] {
                let candidate = vendor.join(target).join(directory).join(name);
                if is_native(&candidate) {
                    return candidate.canonicalize().ok();
                }
            }
        }
    }
    None
}

fn is_native(path: &Path) -> bool {
    let mut magic = [0_u8; 4];
    if std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .is_err()
    {
        return false;
    }
    if cfg!(windows) {
        magic.starts_with(b"MZ")
    } else if cfg!(target_os = "macos") {
        matches!(
            magic,
            [0xcf, 0xfa, 0xed, 0xfe] | [0xca, 0xfe, 0xba, 0xbe] | [0xfe, 0xed, 0xfa, 0xcf]
        )
    } else {
        magic == *b"\x7fELF"
    }
}

fn platform() -> Option<(&'static str, &'static str, &'static str)> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some(("codex-linux-x64", "x86_64-unknown-linux-musl", "codex")),
        ("linux", "aarch64") => Some(("codex-linux-arm64", "aarch64-unknown-linux-musl", "codex")),
        ("windows", "x86_64") => Some(("codex-win32-x64", "x86_64-pc-windows-msvc", "codex.exe")),
        ("windows", "aarch64") => {
            Some(("codex-win32-arm64", "aarch64-pc-windows-msvc", "codex.exe"))
        }
        ("macos", "aarch64") => Some(("codex-darwin-arm64", "aarch64-apple-darwin", "codex")),
        ("macos", "x86_64") => Some(("codex-darwin-x64", "x86_64-apple-darwin", "codex")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_executable() {
        assert!(resolve_native(Path::new("/missing-codex-fixture")).is_none());
    }
    #[test]
    fn recognizes_native_test_executable() {
        assert!(is_native(&std::env::current_exe().unwrap()));
    }

    #[test]
    fn installs_a_recognizable_process_alias_once() {
        let root = tempfile::tempdir().unwrap();
        let source = std::env::current_exe().unwrap();
        let first = install_process_alias(&source, root.path()).unwrap();
        let second = install_process_alias(&source, root.path()).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.file_name().unwrap(), CODEX_PROCESS_FILE_NAME);
        assert!(is_native(&first));
        assert_eq!(
            std::fs::read(source).unwrap(),
            std::fs::read(first).unwrap()
        );
    }
}
