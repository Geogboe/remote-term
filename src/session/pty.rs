use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::platform::command;

use super::{PtyCommand, scrollback::Scrollback};

pub struct PtyHandle {
    _writer_thread: std::thread::JoinHandle<()>,
}

pub struct SpawnConfig<'a> {
    pub command: &'a [String],
    pub input_rx: mpsc::UnboundedReceiver<PtyCommand>,
    pub output_tx: broadcast::Sender<Vec<u8>>,
    pub scrollback: Scrollback,
    pub exit_marker: &'a str,
    pub response_tx: mpsc::UnboundedSender<PtyCommand>,
    pub synthesize_terminal_responses: bool,
    pub exit_tx: oneshot::Sender<u8>,
}

impl PtyHandle {
    pub fn join_writer(self) {
        drop(self);
    }
}

pub fn spawn(config: SpawnConfig<'_>) -> anyhow::Result<PtyHandle> {
    let SpawnConfig {
        command,
        input_rx,
        output_tx,
        scrollback,
        exit_marker,
        response_tx,
        synthesize_terminal_responses,
        exit_tx,
    } = config;

    anyhow::ensure!(!command.is_empty(), "command is required");

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(current_pty_size())
        .context("openpty failed")?;
    let exit_file = exit_signal_path(exit_marker);
    let _ = std::fs::remove_file(&exit_file);
    let resolved_command = command::resolve(command)?;

    let mut builder = CommandBuilder::new(std::env::current_exe()?);
    builder.cwd(std::env::current_dir().context("failed to determine child working directory")?);
    builder.env("RTERM_EXIT_FILE", &exit_file);
    builder.arg("__rterm-child");
    builder.arg(exit_marker);
    builder.arg("--");
    builder.arg(&resolved_command.program);
    for arg in resolved_command.args {
        builder.arg(arg);
    }

    let mut child = pair
        .slave
        .spawn_command(builder)
        .with_context(|| format!("failed to spawn child command `{}`", command[0]))?;
    drop(pair.slave);

    let master: Arc<Mutex<Box<dyn MasterPty + Send>>> = Arc::new(Mutex::new(pair.master));
    let mut reader = master
        .lock()
        .expect("pty master mutex poisoned")
        .try_clone_reader()
        .context("failed to clone PTY reader")?;
    let mut writer = master
        .lock()
        .expect("pty master mutex poisoned")
        .take_writer()
        .context("failed to create PTY writer")?;

    let exit_sender = Arc::new(Mutex::new(Some(exit_tx)));
    let reader_exit_sender = Arc::clone(&exit_sender);
    let reader_exit_marker = exit_marker.as_bytes().to_vec();

    std::thread::Builder::new()
        .name("rterm-pty-reader".to_string())
        .spawn(move || {
            let mut buf = [0_u8; 8192];
            let mut responder = TerminalResponder::new(response_tx, synthesize_terminal_responses);
            let mut filter = ExitMarkerFilter::new(reader_exit_marker);
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        for event in filter.finish() {
                            if let ReaderEvent::Output(bytes) = event {
                                responder.observe(&bytes);
                                scrollback.push(&bytes);
                                let _ = output_tx.send(bytes);
                            }
                        }
                        break;
                    }
                    Ok(n) => {
                        for event in filter.push(&buf[..n]) {
                            match event {
                                ReaderEvent::Output(bytes) => {
                                    responder.observe(&bytes);
                                    scrollback.push(&bytes);
                                    let _ = output_tx.send(bytes);
                                }
                                ReaderEvent::Exit(code) => {
                                    if let Some(tx) = reader_exit_sender
                                        .lock()
                                        .expect("exit mutex poisoned")
                                        .take()
                                    {
                                        let _ = tx.send(code);
                                    }
                                }
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
        })
        .context("failed to spawn PTY reader thread")?;

    let wait_exit_sender = Arc::clone(&exit_sender);
    std::thread::Builder::new()
        .name("rterm-child-wait".to_string())
        .spawn(move || {
            let code = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break exit_code_u8(status.exit_code()),
                    Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                    Err(_) => break 1,
                }
            };
            if let Some(tx) = wait_exit_sender.lock().expect("exit mutex poisoned").take() {
                let _ = tx.send(code);
            }
        })
        .context("failed to spawn child wait thread")?;

    let file_exit_sender = Arc::clone(&exit_sender);
    std::thread::Builder::new()
        .name("rterm-exit-file-watch".to_string())
        .spawn(move || {
            loop {
                if let Ok(code_text) = std::fs::read_to_string(&exit_file)
                    && let Ok(code) = code_text.trim().parse::<u8>()
                {
                    let _ = std::fs::remove_file(&exit_file);
                    if let Some(tx) = file_exit_sender.lock().expect("exit mutex poisoned").take() {
                        let _ = tx.send(code);
                    }
                    break;
                }

                if file_exit_sender
                    .lock()
                    .expect("exit mutex poisoned")
                    .is_none()
                {
                    break;
                }

                std::thread::sleep(Duration::from_millis(50));
            }
        })
        .context("failed to spawn exit file watch thread")?;

    let writer_master = Arc::clone(&master);
    let writer_thread = std::thread::Builder::new()
        .name("rterm-pty-writer".to_string())
        .spawn(move || writer_loop(input_rx, &mut writer, writer_master))
        .context("failed to spawn PTY writer thread")?;

    Ok(PtyHandle {
        _writer_thread: writer_thread,
    })
}

fn writer_loop(
    mut input_rx: mpsc::UnboundedReceiver<PtyCommand>,
    writer: &mut Box<dyn Write + Send>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
) {
    while let Some(command) = input_rx.blocking_recv() {
        match command {
            PtyCommand::Input(bytes) => {
                if writer.write_all(&bytes).is_err() {
                    break;
                }
                let _ = writer.flush();
            }
            PtyCommand::Resize { cols, rows } => {
                let _ = master
                    .lock()
                    .expect("pty master mutex poisoned")
                    .resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
            }
        }
    }
}

pub fn start_local_bridge(
    input_tx: mpsc::UnboundedSender<PtyCommand>,
    mut output_rx: broadcast::Receiver<Vec<u8>>,
    attach_input: bool,
) -> Vec<JoinHandle<()>> {
    if attach_input {
        let _ = std::thread::Builder::new()
            .name("rterm-local-stdin".to_string())
            .spawn(move || {
                let mut stdin = std::io::stdin().lock();
                let mut buf = [0_u8; 8192];
                loop {
                    match stdin.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if input_tx.send(PtyCommand::Input(buf[..n].to_vec())).is_err() {
                                break;
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(_) => break,
                    }
                }
            });
    }

    let output_handle = tokio::spawn(async move {
        loop {
            match output_rx.recv().await {
                Ok(bytes) => {
                    let mut stdout = std::io::stdout().lock();
                    if stdout.write_all(&bytes).is_err() {
                        break;
                    }
                    let _ = stdout.flush();
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    vec![output_handle]
}

pub fn start_resize_watcher(
    input_tx: mpsc::UnboundedSender<PtyCommand>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut last = None;
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            let Ok((cols, rows)) = crossterm::terminal::size() else {
                continue;
            };
            if last == Some((cols, rows)) {
                continue;
            }
            last = Some((cols, rows));
            if input_tx.send(PtyCommand::Resize { cols, rows }).is_err() {
                break;
            }
        }
    })
}

fn current_pty_size() -> PtySize {
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn exit_code_u8(code: u32) -> u8 {
    u8::try_from(code).unwrap_or(1)
}

fn exit_signal_path(marker: &str) -> PathBuf {
    let safe_marker = marker
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    std::env::temp_dir().join(format!(
        "rterm-exit-{}-{}.txt",
        safe_marker,
        std::process::id()
    ))
}

struct TerminalResponder {
    response_tx: mpsc::UnboundedSender<PtyCommand>,
    enabled: bool,
    tail: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
enum ReaderEvent {
    Output(Vec<u8>),
    Exit(u8),
}

struct ExitMarkerFilter {
    prefix: Vec<u8>,
    pending: Vec<u8>,
}

impl ExitMarkerFilter {
    fn new(marker: Vec<u8>) -> Self {
        let mut prefix = b"\x1b]6973;rterm-exit:".to_vec();
        prefix.extend(marker);
        prefix.push(b':');
        Self {
            prefix,
            pending: Vec::new(),
        }
    }

    fn push(&mut self, bytes: &[u8]) -> Vec<ReaderEvent> {
        self.pending.extend_from_slice(bytes);
        let mut events = Vec::new();

        loop {
            let Some(start) = find_subslice(&self.pending, &self.prefix) else {
                let keep = partial_prefix_suffix_len(&self.pending, &self.prefix);
                let emit_len = self.pending.len().saturating_sub(keep);
                if emit_len > 0 {
                    events.push(ReaderEvent::Output(
                        self.pending.drain(..emit_len).collect(),
                    ));
                }
                return events;
            };

            if start > 0 {
                events.push(ReaderEvent::Output(self.pending.drain(..start).collect()));
            }

            let Some(end) = self.pending[self.prefix.len()..]
                .iter()
                .position(|byte| *byte == b'\x07')
                .map(|pos| pos + self.prefix.len())
            else {
                return events;
            };

            let code_bytes = self.pending[self.prefix.len()..end].to_vec();
            self.pending.drain(..=end);
            if let Ok(code_text) = std::str::from_utf8(&code_bytes)
                && let Ok(code) = code_text.parse::<u8>()
            {
                events.push(ReaderEvent::Exit(code));
            }
        }
    }

    fn finish(&mut self) -> Vec<ReaderEvent> {
        if self.pending.is_empty() {
            Vec::new()
        } else {
            vec![ReaderEvent::Output(self.pending.drain(..).collect())]
        }
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn partial_prefix_suffix_len(haystack: &[u8], prefix: &[u8]) -> usize {
    let max = haystack.len().min(prefix.len().saturating_sub(1));
    for len in (1..=max).rev() {
        if haystack[haystack.len() - len..] == prefix[..len] {
            return len;
        }
    }
    0
}

impl TerminalResponder {
    fn new(response_tx: mpsc::UnboundedSender<PtyCommand>, enabled: bool) -> Self {
        Self {
            response_tx,
            enabled,
            tail: Vec::new(),
        }
    }

    fn observe(&mut self, bytes: &[u8]) {
        if !self.enabled {
            return;
        }

        let mut combined = Vec::with_capacity(self.tail.len() + bytes.len());
        combined.extend_from_slice(&self.tail);
        combined.extend_from_slice(bytes);

        if combined.windows(4).any(|window| window == b"\x1b[6n") {
            let _ = self
                .response_tx
                .send(PtyCommand::Input(b"\x1b[1;1R".to_vec()));
        }

        self.tail = combined
            .into_iter()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_responder_answers_cursor_position_query() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut responder = TerminalResponder::new(tx, true);

        responder.observe(b"\x1b[");
        responder.observe(b"6n");

        match rx.try_recv().unwrap() {
            PtyCommand::Input(bytes) => assert_eq!(bytes, b"\x1b[1;1R"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn exit_marker_filter_removes_marker_and_reports_code() {
        let mut filter = ExitMarkerFilter::new(b"abc".to_vec());
        let events = filter.push(b"before\x1b]6973;rterm-exit:abc:7\x07after");

        assert_eq!(
            events,
            vec![
                ReaderEvent::Output(b"before".to_vec()),
                ReaderEvent::Exit(7),
                ReaderEvent::Output(b"after".to_vec())
            ]
        );
    }

    #[test]
    fn exit_marker_filter_handles_split_marker() {
        let mut filter = ExitMarkerFilter::new(b"abc".to_vec());

        assert_eq!(
            filter.push(b"before\x1b]6973;rterm"),
            vec![ReaderEvent::Output(b"before".to_vec())]
        );
        assert_eq!(
            filter.push(b"-exit:abc:42\x07"),
            vec![ReaderEvent::Exit(42)]
        );
    }
}
