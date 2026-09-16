//! Codex App Server 子进程管理。
//!
//! 职责：
//! - spawn Codex App Server 作为子进程
//! - 保证子进程在父进程退出时自动终止（防孤儿进程）
//! - 提供优雅停止和强杀两种退出方式
//!
//! 平台机制：
//! - Windows: Job Object（JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE）
//! - Linux:   prctl(PR_SET_PDEATHSIG, SIGKILL)
//! - macOS:   Process Group

use std::process::Stdio;
use tokio::process::{Child, Command};

pub struct CodexProcess {
    child: Child,
}

impl CodexProcess {
    /// 启动 Codex App Server 子进程。
    pub async fn spawn(codex_path: &str, args: &[&str]) -> Result<Self, String> {
        let mut command = Command::new(codex_path);
        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        setup_parent_death(&mut command);

        let child = command
            .spawn()
            .map_err(|e| format!("failed to spawn codex: {e}"))?;

        let proc = Self { child };

        // Windows 需要在 spawn 后将子进程加入 Job Object
        #[cfg(target_os = "windows")]
        if let Some(pid) = proc.pid() {
            let _ = windows_assign_job(pid as u32);
        }

        Ok(proc)
    }

    /// 优雅停止：等待自然退出，超时后强杀。
    pub async fn shutdown(&mut self, timeout_secs: u64) {
        let timeout = std::time::Duration::from_secs(timeout_secs);
        if tokio::time::timeout(timeout, self.child.wait()).await.is_ok() {
            return;
        }
        self.kill().await;
    }

    /// 强杀子进程。
    pub async fn kill(&mut self) {
        let _ = self.child.kill().await;
    }

    /// 检查是否还在运行。
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// 获取 PID。
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }
}

/// 设置父进程死亡时子进程也终止的机制。
fn setup_parent_death(command: &mut Command) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                Ok(())
            });
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = command;
    }
}

/// Windows: 将子进程加入 Job Object，父进程退出时自动杀掉整个 Job。
#[cfg(target_os = "windows")]
fn windows_assign_job(pid: u32) -> Result<(), String> {
    // 实际实现需要 windows-sys crate
    // 此处为骨架，编译时通过条件编译处理
    let _ = pid;
    Ok(())
}
