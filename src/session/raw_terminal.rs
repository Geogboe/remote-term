use std::io::IsTerminal;

#[derive(Debug)]
pub struct RawTerminalGuard {
    enabled: bool,
}

pub fn is_interactive_terminal() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

impl RawTerminalGuard {
    pub fn enter_if_terminal() -> anyhow::Result<Self> {
        let enabled = is_interactive_terminal();
        if enabled {
            crossterm::terminal::enable_raw_mode()?;
        }

        Ok(Self { enabled })
    }
}

impl Drop for RawTerminalGuard {
    fn drop(&mut self) {
        if self.enabled {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
}
