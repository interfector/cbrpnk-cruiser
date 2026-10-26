use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtyPair, PtySize};

const READ_CHUNK: usize = 8192;
const EVENT_BUFFER: usize = 256;
const INPUT_BUFFER: usize = 64;

#[derive(Debug)]
pub enum PtyEvent {
    Output(Vec<u8>),
    Exited {
        success: bool,
        code: Option<i32>,
        signal: Option<String>,
    },
    Error(String),
}

pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    input: SyncSender<Vec<u8>>,
    events: Receiver<PtyEvent>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    pid: Option<u32>,
}

impl PtySession {
    pub fn spawn_shell(shell: Option<&Path>, cwd: &Path, rows: u16, cols: u16) -> Result<Self> {
        let mut command = shell_command(shell);
        command.cwd(cwd);
        Self::spawn(command, rows, cols)
    }

    pub fn spawn_program(
        program: &Path,
        args: &[OsString],
        cwd: &Path,
        rows: u16,
        cols: u16,
    ) -> Result<Self> {
        let mut command = CommandBuilder::new(program);
        command.args(args);
        command.cwd(cwd);
        Self::spawn(command, rows, cols)
    }

    fn spawn(mut command: CommandBuilder, rows: u16, cols: u16) -> Result<Self> {
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "cbrpnk-cruiser");

        let size = PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = native_pty_system()
            .openpty(size)
            .context("failed to open a PTY")?;
        let PtyPair { slave, master } = pair;
        let child = slave
            .spawn_command(command)
            .context("failed to spawn in PTY")?;
        drop(slave);

        let pid = child.process_id();
        let mut killer = child.clone_killer();
        let reader = master
            .try_clone_reader()
            .context("failed to read from PTY")?;
        let writer = master.take_writer().context("failed to write to PTY")?;

        let (input_tx, input_rx) = mpsc::sync_channel::<Vec<u8>>(INPUT_BUFFER);
        let (event_tx, event_rx) = mpsc::sync_channel::<PtyEvent>(EVENT_BUFFER);

        let reader_events = event_tx.clone();
        if let Err(error) = thread::Builder::new()
            .name("cbrpnk-pty-read".to_string())
            .spawn(move || read_loop(reader, reader_events))
        {
            let _ = killer.kill();
            return Err(error).context("failed to start PTY reader");
        }

        if let Err(error) = thread::Builder::new()
            .name("cbrpnk-pty-write".to_string())
            .spawn(move || write_loop(writer, input_rx))
        {
            let _ = killer.kill();
            return Err(error).context("failed to start PTY writer");
        }

        let mut child = child;
        if let Err(error) = thread::Builder::new()
            .name("cbrpnk-pty-wait".to_string())
            .spawn(move || {
                let event = match child.wait() {
                    Ok(status) => PtyEvent::Exited {
                        success: status.success(),
                        code: i32::try_from(status.exit_code()).ok(),
                        signal: status.signal().map(str::to_string),
                    },
                    Err(error) => PtyEvent::Error(format!("wait failed: {error}")),
                };
                let _ = event_tx.send(event);
            })
        {
            let _ = killer.kill();
            return Err(error).context("failed to start PTY waiter");
        }

        Ok(Self {
            master,
            input: input_tx,
            events: event_rx,
            killer,
            pid,
        })
    }

    pub fn write(&self, bytes: &[u8]) -> bool {
        self.input.try_send(bytes.to_vec()).is_ok()
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        let size = PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        };
        if let Err(error) = self.master.resize(size) {
            log::warn!("PTY resize failed: {error}");
        }
    }

    pub fn try_recv(&self) -> Option<PtyEvent> {
        self.events.try_recv().ok()
    }

    pub fn pid(&self) -> Option<u32> {
        self.pid
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        let _ = self.killer.kill();
    }
}

fn shell_command(shell: Option<&Path>) -> CommandBuilder {
    match shell {
        Some(shell) => CommandBuilder::new(shell),
        None => {
            #[cfg(unix)]
            {
                CommandBuilder::new("/bin/sh")
            }
            #[cfg(not(unix))]
            {
                CommandBuilder::new_default_prog()
            }
        }
    }
}

fn read_loop(mut reader: Box<dyn Read + Send>, events: SyncSender<PtyEvent>) {
    let mut buffer = [0_u8; READ_CHUNK];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                if events
                    .send(PtyEvent::Output(buffer[..read].to_vec()))
                    .is_err()
                {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                let _ = events.send(PtyEvent::Error(format!("read failed: {error}")));
                break;
            }
        }
    }
}

fn write_loop(mut writer: Box<dyn Write + Send>, input: Receiver<Vec<u8>>) {
    while let Ok(bytes) = input.recv() {
        if writer
            .write_all(&bytes)
            .and_then(|()| writer.flush())
            .is_err()
        {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[cfg(unix)]
    #[test]
    fn pty_runs_a_command_and_reports_output_and_exit() {
        let session = PtySession::spawn_program(
            Path::new("/bin/echo"),
            &[OsString::from("pty-ready")],
            Path::new("/tmp"),
            24,
            80,
        )
        .expect("spawn PTY command");

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = Vec::new();
        let mut exited = None;
        while Instant::now() < deadline && exited.is_none() {
            match session.try_recv() {
                Some(PtyEvent::Output(bytes)) => output.extend_from_slice(&bytes),
                Some(PtyEvent::Exited {
                    success,
                    code,
                    signal,
                }) => {
                    exited = Some((success, code, signal));
                }
                Some(PtyEvent::Error(error)) => panic!("PTY error: {error}"),
                None => thread::sleep(Duration::from_millis(5)),
            }
        }

        assert!(String::from_utf8_lossy(&output).contains("pty-ready"));
        assert_eq!(exited, Some((true, Some(0), None)));
    }

    #[cfg(unix)]
    #[test]
    fn pty_resize_is_nonzero_clamped() {
        let session =
            PtySession::spawn_program(Path::new("/bin/echo"), &[], Path::new("/tmp"), 1, 1)
                .expect("spawn PTY command");
        session.resize(0, 0);
    }
}
