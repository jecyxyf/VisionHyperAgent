//! Process primitives only. The server decides when to spawn, monitor, and shut down.
use std::collections::VecDeque;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;

#[cfg(windows)]
#[path = "process/windows.rs"]
mod windows;

/// Environment values can contain per-start gateway credentials, so no Debug impl is derived.
#[derive(Clone)]
pub struct ProcessConfig {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(OsString, OsString)>,
}

pub struct CodexProcess {
    child: Child,
    output_tasks: Vec<JoinHandle<()>>,
    diagnostics: Arc<Mutex<VecDeque<&'static str>>>,
    #[cfg(windows)]
    job: windows::Job,
}

impl CodexProcess {
    pub async fn spawn(config: &ProcessConfig) -> io::Result<Self> {
        let mut command = Command::new(&config.executable);
        command
            .args(&config.args)
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY")
            .env_remove("OPENAI_BASE_URL")
            .envs(config.env.iter().cloned())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = &config.cwd {
            command.current_dir(cwd);
        }
        setup_process(&mut command);
        #[cfg(windows)]
        let job = windows::Job::new()?;
        let mut child = command.spawn()?;
        #[cfg(windows)]
        {
            // Created suspended: no descendant can escape before job assignment.
            let result = job.assign_and_resume(
                child
                    .id()
                    .ok_or_else(|| io::Error::other("child has no PID"))?,
            );
            rollback_failed_setup(&mut child, result).await?;
        }
        let diagnostics = Arc::new(Mutex::new(VecDeque::new()));
        let mut output_tasks = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            output_tasks.push(tokio::spawn(drain(stdout, "stdout", diagnostics.clone())));
        }
        if let Some(stderr) = child.stderr.take() {
            output_tasks.push(tokio::spawn(drain(stderr, "stderr", diagnostics.clone())));
        }
        log::info!("Codex child started, pid={:?}", child.id());
        Ok(Self {
            child,
            output_tasks,
            diagnostics,
            #[cfg(windows)]
            job,
        })
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    /// Classified diagnostics only. Never return raw child output to the UI or logs.
    pub fn diagnostics(&self) -> Vec<&'static str> {
        self.diagnostics.lock().unwrap().iter().copied().collect()
    }

    pub async fn shutdown(&mut self, grace: Duration) -> io::Result<ExitStatus> {
        if let Some(status) = self.child.try_wait()? {
            self.stop_readers();
            return Ok(status);
        }
        #[cfg(unix)]
        if let Some(pid) = self.pid() {
            // We created a new session, and the child has not been reaped: this is our group.
            unsafe {
                libc::kill(-(pid as i32), libc::SIGTERM);
            }
        }
        let result = match tokio::time::timeout(grace, self.child.wait()).await {
            Ok(result) => result,
            Err(_) => self.kill().await,
        };
        self.stop_readers();
        log::info!("Codex child reaped, success={}", result.is_ok());
        result
    }

    pub async fn kill(&mut self) -> io::Result<ExitStatus> {
        if let Some(status) = self.child.try_wait()? {
            self.stop_readers();
            return Ok(status);
        }
        #[cfg(unix)]
        if let Some(pid) = self.pid() {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
        #[cfg(windows)]
        self.job.terminate()?;
        // kill also waits/reaps. start_kill followed by wait lets us return the actual status.
        self.child.start_kill()?;
        let result = self.child.wait().await;
        self.stop_readers();
        result
    }

    fn stop_readers(&mut self) {
        for task in self.output_tasks.drain(..) {
            task.abort();
        }
    }
}

impl Drop for CodexProcess {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            #[cfg(unix)]
            if let Some(pid) = self.pid() {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
            let _ = self.child.start_kill();
        }
        self.stop_readers();
        // Tokio reaps the direct child; on Windows closing the job kills all remaining members.
    }
}

fn setup_process(command: &mut Command) {
    #[cfg(unix)]
    {
        #[cfg(target_os = "linux")]
        let parent = unsafe { libc::getpid() };
        // Only async-signal-safe syscalls in the post-fork/pre-exec closure.
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                #[cfg(target_os = "linux")]
                {
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                        return Err(io::Error::last_os_error());
                    }
                    if libc::getppid() != parent {
                        return Err(io::Error::other("parent exited during child setup"));
                    }
                }
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED};
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
    }
    #[cfg(not(any(unix, windows)))]
    let _ = command;
}

async fn drain(
    mut reader: impl AsyncRead + Unpin,
    stream: &'static str,
    diagnostics: Arc<Mutex<VecDeque<&'static str>>>,
) {
    let mut bytes = [0; 4096];
    let mut total = 0_u64;
    loop {
        match reader.read(&mut bytes).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                total += n as u64;
                // Chunk-bounded parsing; no unbounded line buffer and no raw prompt/secret logging.
                let text = String::from_utf8_lossy(&bytes[..n]).to_lowercase();
                let category = if text.contains("address already in use") {
                    Some("Codex listening port is already in use")
                } else if text.contains("failed to parse") || text.contains("error loading config")
                {
                    Some("Codex configuration could not be parsed")
                } else if text.contains("api key") || text.contains("authentication") {
                    Some("Codex provider authentication requires attention")
                } else if text.contains("unexpected argument") {
                    Some("Codex executable does not support the configured arguments")
                } else {
                    None
                };
                if let Some(category) = category {
                    let mut recent = diagnostics.lock().unwrap();
                    if !recent.contains(&category) {
                        if recent.len() == 16 {
                            recent.pop_front();
                        }
                        recent.push_back(category);
                        log::warn!("{category}");
                    }
                }
            }
        }
    }
    log::debug!("Codex {stream} drain finished, bytes={total}");
}

// Shared with the Windows path so failure ordering/rollback can be tested without Win32.
#[cfg(any(windows, test))]
fn assign_before_resume(
    assign: impl FnOnce() -> io::Result<()>,
    resume: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    assign()?;
    resume()
}

#[cfg(any(windows, test))]
async fn rollback_failed_setup(child: &mut Child, result: io::Result<()>) -> io::Result<()> {
    if let Err(error) = result {
        let _ = child.kill().await;
        let _ = child.wait().await;
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod startup_tests {
    use super::*;
    #[test]
    fn assignment_failure_never_resumes_the_suspended_child() {
        let resumed = std::cell::Cell::new(false);
        let result = assign_before_resume(
            || Err(io::Error::other("assignment failed")),
            || {
                resumed.set(true);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(!resumed.get());
    }
    #[test]
    fn assignment_precedes_resume_and_resume_errors_are_not_ignored() {
        let steps = std::cell::RefCell::new(Vec::new());
        let result = assign_before_resume(
            || {
                steps.borrow_mut().push("assigned");
                Ok(())
            },
            || {
                steps.borrow_mut().push("resume");
                Err(io::Error::other("resume failed"))
            },
        );
        assert!(result.is_err());
        assert_eq!(*steps.borrow(), vec!["assigned", "resume"]);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn failed_platform_setup_kills_and_reaps_the_created_child() {
        for message in ["assignment failed", "resume failed"] {
            let mut child = Command::new("/bin/sleep")
                .arg("60")
                .kill_on_drop(true)
                .spawn()
                .unwrap();
            let result = rollback_failed_setup(&mut child, Err(io::Error::other(message))).await;
            assert_eq!(result.unwrap_err().to_string(), message);
            assert!(child.try_wait().unwrap().is_some());
        }
    }
}
