use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use vha_codex_agent::{CodexProcess, ProcessConfig};

struct TempRoot(PathBuf);

struct FixtureParent(std::process::Child);
impl Drop for FixtureParent {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl TempRoot {
    fn new() -> Self {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("vha-process-test-{}-{id}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn helper_config(mode: &str, dir: &Path) -> ProcessConfig {
    ProcessConfig {
        executable: std::env::current_exe().unwrap(),
        args: vec![
            "--exact".into(),
            "process_fixture".into(),
            "--nocapture".into(),
        ],
        cwd: Some(dir.into()),
        env: vec![
            ("VHA_TEST_PROCESS_MODE".into(), mode.into()),
            ("VHA_TEST_PROCESS_DIR".into(), dir.as_os_str().into()),
        ],
    }
}

fn is_running(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            return false;
        };
        !matches!(
            stat.rsplit_once(") ").and_then(|(_, s)| s.chars().next()),
            Some('Z' | 'X')
        )
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return false;
        }
        let mut code = 0;
        let success = unsafe { GetExitCodeProcess(process, &mut code) } != 0;
        unsafe {
            CloseHandle(process);
        }
        success && code == STILL_ACTIVE as u32
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    unsafe {
        libc::kill(pid as i32, 0) == 0
    }
}

async fn wait_file(path: &Path) -> String {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(s) = std::fs::read_to_string(path) {
                if !s.trim().is_empty() {
                    return s;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

async fn wait_dead(pid: u32) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while is_running(pid) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("owned child must terminate");
}

// Runs normally as a no-op. Child-only modes exercise real process behavior on both platforms.
#[test]
fn process_fixture() {
    let Ok(mode) = std::env::var("VHA_TEST_PROCESS_MODE") else {
        return;
    };
    let dir = PathBuf::from(std::env::var_os("VHA_TEST_PROCESS_DIR").unwrap());
    if mode == "flood" {
        use std::io::Write;
        for _ in 0..128 {
            std::io::stdout().write_all(&[b'x'; 8192]).unwrap();
            std::io::stderr().write_all(&[b'y'; 8192]).unwrap();
        }
        std::fs::write(dir.join("drained"), "ok").unwrap();
    }
    if mode == "diagnostics" {
        use std::io::Write;
        std::io::stderr()
            .write_all(b"address already in use CREDENTIAL_MARKER\n")
            .unwrap();
        std::io::stderr().flush().unwrap();
        std::fs::write(dir.join("ready"), "ok").unwrap();
    }
    if mode == "exit" {
        std::process::exit(7);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let _child = if mode == "parent" {
            let child = CodexProcess::spawn(&helper_config("sleep", &dir))
                .await
                .unwrap();
            std::fs::write(dir.join("child.pid"), child.pid().unwrap().to_string()).unwrap();
            Some(child)
        } else {
            None
        };
        if mode == "grandparent" {
            let child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "process_fixture", "--nocapture"])
                .env("VHA_TEST_PROCESS_MODE", "sleep")
                .spawn()
                .unwrap();
            std::fs::write(dir.join("grandchild.pid"), child.id().to_string()).unwrap();
            // Kept alive until the enclosing process group / Job Object is terminated.
            std::mem::forget(child);
        }
        std::future::pending::<()>().await;
    });
}

#[tokio::test]
async fn explicit_stop_reaps_only_the_owned_child() {
    let dir = TempRoot::new();
    let mut owned = CodexProcess::spawn(&helper_config("sleep", &dir.0))
        .await
        .unwrap();
    let mut unrelated = CodexProcess::spawn(&helper_config("sleep", &dir.0))
        .await
        .unwrap();
    let pid = owned.pid().unwrap();
    let other_pid = unrelated.pid().unwrap();
    assert!(is_running(pid) && is_running(other_pid));
    owned.shutdown(Duration::from_millis(150)).await.unwrap();
    wait_dead(pid).await;
    assert!(
        is_running(other_pid),
        "shutdown must not kill other Codex instances"
    );
    assert!(owned.try_wait().unwrap().is_some());
    owned.shutdown(Duration::from_millis(20)).await.unwrap();
    unrelated.kill().await.unwrap();
}

#[tokio::test]
async fn drop_kills_child_instead_of_detaching_it() {
    let dir = TempRoot::new();
    let process = CodexProcess::spawn(&helper_config("sleep", &dir.0))
        .await
        .unwrap();
    let pid = process.pid().unwrap();
    drop(process);
    wait_dead(pid).await;
}

#[tokio::test]
async fn process_group_or_job_also_stops_descendants() {
    let dir = TempRoot::new();
    let mut process = CodexProcess::spawn(&helper_config("grandparent", &dir.0))
        .await
        .unwrap();
    let pid = wait_file(&dir.0.join("grandchild.pid"))
        .await
        .parse::<u32>()
        .unwrap();
    assert!(is_running(pid));
    process.shutdown(Duration::from_millis(100)).await.unwrap();
    wait_dead(pid).await;
}

#[tokio::test]
async fn stdout_and_stderr_are_drained_without_pipe_deadlock() {
    let dir = TempRoot::new();
    let mut process = CodexProcess::spawn(&helper_config("flood", &dir.0))
        .await
        .unwrap();
    assert_eq!(wait_file(&dir.0.join("drained")).await, "ok");
    process.shutdown(Duration::from_millis(100)).await.unwrap();
}

#[tokio::test]
async fn diagnostics_do_not_reveal_raw_child_output() {
    let dir = TempRoot::new();
    let mut process = CodexProcess::spawn(&helper_config("diagnostics", &dir.0))
        .await
        .unwrap();
    wait_file(&dir.0.join("ready")).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while process.diagnostics().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        process.diagnostics(),
        vec!["Codex listening port is already in use"]
    );
    process.shutdown(Duration::from_millis(100)).await.unwrap();
}

#[tokio::test]
async fn early_exit_preserves_status_and_shutdown_is_safe() {
    let dir = TempRoot::new();
    let mut process = CodexProcess::spawn(&helper_config("exit", &dir.0))
        .await
        .unwrap();
    let status = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(status) = process.try_wait().unwrap() {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(status.code(), Some(7));
    assert_eq!(
        process
            .shutdown(Duration::from_millis(50))
            .await
            .unwrap()
            .code(),
        Some(7)
    );
}

#[tokio::test]
async fn nonexistent_executable_is_a_startup_error() {
    let dir = TempRoot::new();
    let mut config = helper_config("sleep", &dir.0);
    config.executable = dir.0.join("not-a-real-executable");
    assert!(CodexProcess::spawn(&config).await.is_err());
}

#[cfg(any(target_os = "linux", windows))]
#[tokio::test]
async fn abrupt_parent_death_does_not_leave_direct_child_running() {
    let dir = TempRoot::new();
    // Intentionally outside CodexProcess: kill the parent without invoking any Rust destructor.
    let mut parent = FixtureParent(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "process_fixture", "--nocapture"])
            .env("VHA_TEST_PROCESS_MODE", "parent")
            .env("VHA_TEST_PROCESS_DIR", &dir.0)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let pid = wait_file(&dir.0.join("child.pid"))
        .await
        .parse::<u32>()
        .unwrap();
    assert!(is_running(pid));
    parent.0.kill().unwrap();
    parent.0.wait().unwrap();
    wait_dead(pid).await;
}
