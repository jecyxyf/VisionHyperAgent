//! Resolve the native executable, not npm's Node wrapper (which breaks parent-death ownership).
use std::io::Read;
use std::path::{Path, PathBuf};

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
}
