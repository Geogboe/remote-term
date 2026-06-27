pub mod pty;
pub mod raw_terminal;
pub mod registry;
pub mod scrollback;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::web::server::{self, AppState};

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub command: Vec<String>,
    pub bind_addr: SocketAddr,
    pub lan: bool,
    pub web_write: bool,
    pub max_clients: usize,
    pub once: bool,
    pub headless: bool,
    pub token: String,
    pub word_erase: Vec<u8>,
}

#[derive(Debug)]
pub enum PtyCommand {
    Input(Vec<u8>),
    Resize { cols: u16, rows: u16 },
}

#[derive(Debug)]
pub struct SessionState {
    pub token: String,
    pub web_write: bool,
    pub max_clients: usize,
    pub once: bool,
    pub word_erase: Vec<u8>,
    pub browser_resize: bool,
    pub input_tx: mpsc::UnboundedSender<PtyCommand>,
    pub output_tx: broadcast::Sender<Vec<u8>>,
    pub scrollback: scrollback::Scrollback,
    active_clients: std::sync::atomic::AtomicUsize,
    closed_to_new_clients: std::sync::atomic::AtomicBool,
}

impl SessionState {
    pub fn new(config: &RunConfig, input_tx: mpsc::UnboundedSender<PtyCommand>) -> Arc<Self> {
        let (output_tx, _) = broadcast::channel(256);
        Arc::new(Self {
            token: config.token.clone(),
            web_write: config.web_write,
            max_clients: config.max_clients,
            once: config.once,
            word_erase: config.word_erase.clone(),
            browser_resize: config.headless,
            input_tx,
            output_tx,
            scrollback: scrollback::Scrollback::new(1024 * 1024),
            active_clients: std::sync::atomic::AtomicUsize::new(0),
            closed_to_new_clients: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub fn try_acquire_client(self: &Arc<Self>) -> Option<ClientPermit> {
        use std::sync::atomic::Ordering;

        if self.closed_to_new_clients.load(Ordering::SeqCst) {
            return None;
        }

        loop {
            let current = self.active_clients.load(Ordering::SeqCst);
            if current >= self.max_clients {
                return None;
            }
            if self
                .active_clients
                .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return Some(ClientPermit {
                    state: Arc::clone(self),
                });
            }
        }
    }

    fn release_client(&self) {
        use std::sync::atomic::Ordering;

        self.active_clients.fetch_sub(1, Ordering::SeqCst);
        if self.once {
            self.closed_to_new_clients.store(true, Ordering::SeqCst);
        }
    }

    pub fn active_clients(&self) -> usize {
        self.active_clients
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub struct ClientPermit {
    state: Arc<SessionState>,
}

impl Drop for ClientPermit {
    fn drop(&mut self) {
        self.state.release_client();
    }
}

pub async fn run_session(config: RunConfig) -> anyhow::Result<u8> {
    let (input_tx, input_rx) = mpsc::unbounded_channel();
    let state = SessionState::new(&config, input_tx.clone());
    let listener = server::bind(config.bind_addr)
        .await
        .with_context(|| format!("failed to bind web server at {}", config.bind_addr))?;
    let _registration = registry::register(&config)?;
    let synthesize_terminal_responses = config.headless || !raw_terminal::is_interactive_terminal();

    let _raw = if config.headless {
        None
    } else {
        Some(raw_terminal::RawTerminalGuard::enter_if_terminal()?)
    };

    let interactive_terminal = raw_terminal::is_interactive_terminal();

    let local_handles = if config.headless {
        Vec::new()
    } else {
        pty::start_local_bridge(
            input_tx.clone(),
            state.output_tx.subscribe(),
            interactive_terminal,
        )
    };

    let (exit_tx, exit_rx) = oneshot::channel();
    let pty = pty::spawn(pty::SpawnConfig {
        command: &config.command,
        input_rx,
        output_tx: state.output_tx.clone(),
        scrollback: state.scrollback.clone(),
        exit_marker: &config.token,
        response_tx: input_tx.clone(),
        synthesize_terminal_responses,
        exit_tx,
    })
    .context("failed to spawn PTY child")?;

    let resize_handle = if config.headless {
        None
    } else {
        Some(pty::start_resize_watcher(
            input_tx.clone(),
            Duration::from_millis(250),
        ))
    };

    let server_handle = tokio::spawn(server::serve_listener(
        AppState {
            session: Arc::clone(&state),
        },
        listener,
    ));

    let exit_code = exit_rx.await.unwrap_or(1);

    drop(input_tx);
    pty.join_writer();

    for handle in local_handles {
        handle.abort();
    }
    if let Some(handle) = resize_handle {
        handle.abort();
    }
    server_handle.abort();

    Ok(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> RunConfig {
        RunConfig {
            command: vec!["echo".to_string(), "hi".to_string()],
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 7843)),
            lan: false,
            web_write: false,
            max_clients: 1,
            once: true,
            headless: false,
            token: "abc".to_string(),
            word_erase: vec![0x17],
        }
    }

    #[test]
    fn one_client_default_is_enforced() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let state = SessionState::new(&test_config(), tx);
        let first = state.try_acquire_client();
        assert!(first.is_some());
        assert!(state.try_acquire_client().is_none());
        drop(first);
        assert!(state.try_acquire_client().is_none());
    }
}
