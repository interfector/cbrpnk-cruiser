use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use anyhow::{bail, Context, Result};

use crate::fs::{scan_directory, ScanRequest};
use crate::launch::terminal::{find_program, TerminalCommand};

const MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub label: String,
    pub shell_mode: bool,
}

impl LaunchPlan {
    pub fn display_command(&self) -> String {
        let mut text = self.program.to_string_lossy().into_owned();
        for arg in &self.args {
            text.push(' ');
            text.push_str(&quote_argument(arg));
        }
        text
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone)]
pub enum LaunchEvent {
    Started {
        id: u64,
        label: String,
        pid: u32,
        command: String,
        cwd: PathBuf,
    },
    Output {
        id: u64,
        stream: OutputStream,
        text: String,
    },
    Finished {
        id: u64,
        success: bool,
        code: Option<i32>,
        truncated: bool,
    },
    Failed {
        id: u64,
        message: String,
    },
}

pub fn plan_executable(path: &Path, cwd: &Path, args: Vec<OsString>) -> Result<LaunchPlan> {
    if !path.is_file() {
        bail!("not a regular file: {}", path.display());
    }
    check_executable(path)?;
    Ok(LaunchPlan {
        program: path.to_path_buf(),
        args,
        cwd: cwd.to_path_buf(),
        label: path.display().to_string(),
        shell_mode: false,
    })
}

pub fn plan_open(path: &Path, cwd: &Path) -> Result<LaunchPlan> {
    #[cfg(target_os = "linux")]
    let opener = "xdg-open";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let opener = "";

    if opener.is_empty() {
        bail!("system opener is not supported on this platform");
    }
    let Some(program) = find_program(opener, std::env::var_os("PATH").as_deref()) else {
        bail!("{opener} was not found in PATH");
    };
    Ok(LaunchPlan {
        program,
        args: vec![path.as_os_str().to_os_string()],
        cwd: cwd.to_path_buf(),
        label: format!("open {}", path.display()),
        shell_mode: false,
    })
}

pub fn plan_shell(command: &str, shell: &Path, cwd: &Path) -> LaunchPlan {
    LaunchPlan {
        program: shell.to_path_buf(),
        args: vec![OsString::from("-lc"), OsString::from(command)],
        cwd: cwd.to_path_buf(),
        label: format!("shell: {command}"),
        shell_mode: true,
    }
}

pub fn plan_external(words: &[String], cwd: &Path) -> Result<LaunchPlan> {
    let Some(program) = words.first() else {
        bail!("empty command");
    };
    let program = PathBuf::from(program);
    let args = words.iter().skip(1).map(OsString::from).collect();
    let program = if program.components().count() > 1 {
        program
    } else {
        find_program(
            &program.to_string_lossy(),
            std::env::var_os("PATH").as_deref(),
        )
        .with_context(|| format!("command not found: {}", program.display()))?
    };
    Ok(LaunchPlan {
        program,
        args,
        cwd: cwd.to_path_buf(),
        label: words.join(" "),
        shell_mode: false,
    })
}

pub fn plan_terminal(terminal: TerminalCommand) -> LaunchPlan {
    LaunchPlan {
        program: terminal.program.clone(),
        args: terminal.args,
        cwd: terminal.cwd,
        label: format!("terminal {}", terminal.program.display()),
        shell_mode: false,
    }
}

pub fn spawn_detached(plan: &LaunchPlan) -> Result<u32> {
    let mut command = Command::new(&plan.program);
    command
        .args(&plan.args)
        .current_dir(&plan.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command
        .spawn()
        .with_context(|| format!("failed to launch {}", plan.display_command()))?;
    Ok(child.id())
}

pub struct LaunchWorker {
    sender: Sender<Work>,
    receiver: Receiver<LaunchEvent>,
}

enum Work {
    Plan {
        id: u64,
        plan: LaunchPlan,
    },
    List {
        id: u64,
        path: PathBuf,
        show_hidden: bool,
    },
}

impl LaunchWorker {
    pub fn spawn() -> Self {
        let (work_tx, work_rx) = mpsc::channel::<Work>();
        let (event_tx, event_rx) = mpsc::channel::<LaunchEvent>();
        thread::Builder::new()
            .name("cbrpnk-launch".to_string())
            .spawn(move || {
                while let Ok(work) = work_rx.recv() {
                    match work {
                        Work::Plan { id, plan } => {
                            run_captured(id, plan, event_tx.clone());
                        }
                        Work::List {
                            id,
                            path,
                            show_hidden,
                        } => {
                            list_directory(id, &path, show_hidden, &event_tx);
                        }
                    }
                }
            })
            .ok();
        Self {
            sender: work_tx,
            receiver: event_rx,
        }
    }

    pub fn run(&self, id: u64, plan: LaunchPlan) {
        let _ = self.sender.send(Work::Plan { id, plan });
    }

    pub fn list_directory(&self, id: u64, path: PathBuf, show_hidden: bool) {
        let _ = self.sender.send(Work::List {
            id,
            path,
            show_hidden,
        });
    }

    pub fn try_recv(&self) -> Option<LaunchEvent> {
        self.receiver.try_recv().ok()
    }
}

fn run_captured(id: u64, plan: LaunchPlan, sender: Sender<LaunchEvent>) {
    let mut command = Command::new(&plan.program);
    command
        .args(&plan.args)
        .current_dir(&plan.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = sender.send(LaunchEvent::Failed {
                id,
                message: format!("{}: {error}", plan.display_command()),
            });
            return;
        }
    };
    let pid = child.id();
    let _ = sender.send(LaunchEvent::Started {
        id,
        label: plan.label.clone(),
        pid,
        command: plan.display_command(),
        cwd: plan.cwd.clone(),
    });

    let budget = Arc::new(AtomicUsize::new(0));
    let truncated = Arc::new(AtomicBool::new(false));
    let stdout = child.stdout.take().map(|stream| {
        spawn_reader(
            id,
            OutputStream::Stdout,
            stream,
            sender.clone(),
            Arc::clone(&budget),
            Arc::clone(&truncated),
        )
    });
    let stderr = child.stderr.take().map(|stream| {
        spawn_reader(
            id,
            OutputStream::Stderr,
            stream,
            sender.clone(),
            Arc::clone(&budget),
            Arc::clone(&truncated),
        )
    });

    let status = child.wait();
    if let Some(handle) = stdout {
        let _ = handle.join();
    }
    if let Some(handle) = stderr {
        let _ = handle.join();
    }
    match status {
        Ok(status) => {
            let _ = sender.send(LaunchEvent::Finished {
                id,
                success: status.success(),
                code: status.code(),
                truncated: truncated.load(Ordering::Relaxed),
            });
        }
        Err(error) => {
            let _ = sender.send(LaunchEvent::Failed {
                id,
                message: format!("wait failed: {error}"),
            });
        }
    }
}

fn spawn_reader<R: Read + Send + 'static>(
    id: u64,
    stream: OutputStream,
    reader: R,
    sender: Sender<LaunchEvent>,
    budget: Arc<AtomicUsize>,
    truncated: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    let total = budget.fetch_add(read, Ordering::Relaxed) + read;
                    if total > MAX_OUTPUT_BYTES {
                        if !truncated.swap(true, Ordering::Relaxed) {
                            let _ = sender.send(LaunchEvent::Output {
                                id,
                                stream,
                                text: "[output truncated]".to_string(),
                            });
                        }
                        break;
                    }
                    let text = String::from_utf8_lossy(&buffer)
                        .trim_end_matches(['\r', '\n'])
                        .to_string();
                    if sender
                        .send(LaunchEvent::Output { id, stream, text })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    })
}

fn list_directory(id: u64, path: &Path, show_hidden: bool, sender: &Sender<LaunchEvent>) {
    let request = ScanRequest {
        path: path.to_path_buf(),
        generation: 0,
        show_hidden,
        max_entries: 5000,
    };
    let result = scan_directory(&request);
    if let Some(error) = &result.error {
        let _ = sender.send(LaunchEvent::Failed {
            id,
            message: format!("{}: {error}", path.display()),
        });
        return;
    }
    let _ = sender.send(LaunchEvent::Output {
        id,
        stream: OutputStream::Stdout,
        text: format!("{}:", path.display()),
    });
    for entry in &result.entries {
        let marker = if entry.kind == crate::fs::EntryKind::Directory {
            "/"
        } else if entry.executable {
            "*"
        } else if entry.kind == crate::fs::EntryKind::Symlink {
            "@"
        } else {
            ""
        };
        let _ = sender.send(LaunchEvent::Output {
            id,
            stream: OutputStream::Stdout,
            text: format!("{}{}", entry.display_name(), marker),
        });
    }
    if result.truncated {
        let _ = sender.send(LaunchEvent::Output {
            id,
            stream: OutputStream::Stdout,
            text: "[truncated]".to_string(),
        });
    }
    let _ = sender.send(LaunchEvent::Finished {
        id,
        success: true,
        code: Some(0),
        truncated: result.truncated,
    });
}

fn check_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata =
            std::fs::metadata(path).with_context(|| format!("cannot stat {}", path.display()))?;
        if metadata.permissions().mode() & 0o111 == 0 {
            bail!("not executable: {}", path.display());
        }
    }
    #[cfg(not(unix))]
    {
        let metadata =
            std::fs::metadata(path).with_context(|| format!("cannot stat {}", path.display()))?;
        if !metadata.is_file() {
            bail!("not executable: {}", path.display());
        }
    }
    Ok(())
}

fn quote_argument(argument: &OsStr) -> String {
    let text = argument.to_string_lossy();
    if text
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-_./:=@+".contains(character))
    {
        return text.into_owned();
    }
    format!("'{}'", text.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_executable_and_keeps_argv() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("script file");
        std::fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&file).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&file, permissions).unwrap();
        }
        let plan = plan_executable(&file, dir.path(), vec![OsString::from("a b")]).unwrap();
        assert_eq!(plan.program, file);
        assert_eq!(plan.args, vec![OsString::from("a b")]);
        assert_eq!(plan.cwd, dir.path());
    }

    #[test]
    fn refuses_non_executable() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("plain");
        std::fs::write(&file, "x").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&file).unwrap().permissions();
            permissions.set_mode(0o644);
            std::fs::set_permissions(&file, permissions).unwrap();
            assert!(plan_executable(&file, dir.path(), Vec::new()).is_err());
        }
    }

    #[test]
    fn shell_plan_uses_login_command() {
        let plan = plan_shell("printf hi", Path::new("/bin/sh"), Path::new("/tmp"));
        assert_eq!(plan.args[0], OsString::from("-lc"));
        assert!(plan.shell_mode);
    }

    #[test]
    fn quotes_arguments_for_display() {
        assert_eq!(quote_argument(OsStr::new("hello")), "hello");
        assert_eq!(quote_argument(OsStr::new("hello world")), "'hello world'");
    }

    #[test]
    fn launch_worker_captures_echo() {
        let worker = LaunchWorker::spawn();
        let plan = LaunchPlan {
            program: PathBuf::from("/bin/echo"),
            args: vec![OsString::from("hello")],
            cwd: std::env::temp_dir(),
            label: "echo".to_string(),
            shell_mode: false,
        };
        worker.run(7, plan);
        let mut output = String::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if let Some(event) = worker.try_recv() {
                match event {
                    LaunchEvent::Output { text, .. } => output.push_str(&text),
                    LaunchEvent::Finished { id, success, .. } => {
                        assert_eq!(id, 7);
                        assert!(success);
                        break;
                    }
                    LaunchEvent::Failed { message, .. } => panic!("{message}"),
                    _ => {}
                }
            }
            assert!(std::time::Instant::now() < deadline, "timed out");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(output.contains("hello"));
    }
}
