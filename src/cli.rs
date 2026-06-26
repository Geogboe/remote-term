use std::net::SocketAddr;

use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(
    author,
    version,
    about = "Run a command in a managed PTY with optional browser access"
)]
pub struct Cli {
    /// Bind address for the embedded web server.
    #[arg(long, default_value = "127.0.0.1:7843")]
    pub bind: SocketAddr,

    /// Expose the web server on all interfaces.
    #[arg(long)]
    pub lan: bool,

    /// Allow browser clients to write input to the PTY.
    #[arg(long)]
    pub write: bool,

    /// Maximum number of concurrent browser clients.
    #[arg(long, default_value_t = 1)]
    pub max_clients: usize,

    /// Stop accepting browser clients after the first disconnect.
    #[arg(long)]
    pub once: bool,

    /// Do not attach the local terminal to the PTY.
    #[arg(long)]
    pub headless: bool,

    /// Manually supply the browser URL token.
    #[arg(long)]
    pub token: Option<String>,

    /// Browser Ctrl+Backspace/word-erase byte sequence. Default is Ctrl+W.
    #[arg(long, default_value = "\\x17")]
    pub word_erase: String,

    /// Command and arguments to run after `--`.
    #[arg(last = true, required = true)]
    pub command: Vec<String>,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    pub fn effective_bind(&self) -> SocketAddr {
        if self.lan && self.bind.ip().is_loopback() {
            SocketAddr::from(([0, 0, 0, 0], self.bind.port()))
        } else {
            self.bind
        }
    }

    pub fn decoded_word_erase(&self) -> Vec<u8> {
        decode_escaped_bytes(&self.word_erase)
    }
}

fn decode_escaped_bytes(input: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            let mut buf = [0; 4];
            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            continue;
        }

        match chars.next() {
            Some('n') => out.push(b'\n'),
            Some('r') => out.push(b'\r'),
            Some('t') => out.push(b'\t'),
            Some('\\') => out.push(b'\\'),
            Some('x') => {
                let hi = chars.next();
                let lo = chars.next();
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    let hex = [hi, lo].iter().collect::<String>();
                    if let Ok(value) = u8::from_str_radix(&hex, 16) {
                        out.push(value);
                    }
                }
            }
            Some(other) => {
                out.push(b'\\');
                let mut buf = [0; 4];
                out.extend_from_slice(other.encode_utf8(&mut buf).as_bytes());
            }
            None => out.push(b'\\'),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lan_promotes_loopback_bind_to_all_interfaces() {
        let cli = Cli::try_parse_from(["rterm", "--lan", "--", "codex"]).unwrap();
        assert_eq!(cli.effective_bind(), SocketAddr::from(([0, 0, 0, 0], 7843)));
        assert!(!cli.write);
        assert_eq!(cli.max_clients, 1);
    }

    #[test]
    fn write_is_explicit_and_command_is_collected_after_separator() {
        let cli =
            Cli::try_parse_from(["rterm", "--lan", "--write", "--", "pwsh", "-NoLogo"]).unwrap();
        assert!(cli.write);
        assert_eq!(cli.command, ["pwsh", "-NoLogo"]);
    }

    #[test]
    fn default_word_erase_decodes_to_ctrl_w() {
        let cli = Cli::try_parse_from(["rterm", "--", "codex"]).unwrap();
        assert_eq!(cli.decoded_word_erase(), vec![0x17]);
    }

    #[test]
    fn explicit_word_erase_supports_escape_sequences() {
        let cli =
            Cli::try_parse_from(["rterm", "--word-erase", "\\x1b\\x7f", "--", "bash"]).unwrap();
        assert_eq!(cli.decoded_word_erase(), vec![0x1b, 0x7f]);
    }
}
