use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;
use rterm::cli::Cli;
use rterm::platform::command;
use rterm::platform::{lan_ip, wsl};
use rterm::security::token;
use rterm::session::{RunConfig, run_session};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|arg| arg == "__rterm-child") {
        return child_helper(&args[2..]);
    }

    if let Err(error) = init_tracing() {
        eprintln!("failed to initialize tracing: {error:#}");
    }

    match run().await {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("rterm error: {error:#}");
            ExitCode::from(1)
        }
    }
}

fn child_helper(args: &[String]) -> ExitCode {
    let Some(marker) = args.first() else {
        return ExitCode::from(127);
    };
    let command_start = args.iter().position(|arg| arg == "--").map(|idx| idx + 1);
    let Some(command_start) = command_start else {
        return ExitCode::from(127);
    };
    let command = &args[command_start..];
    if command.is_empty() {
        return ExitCode::from(127);
    }

    let code = match command::child_command(command).and_then(|mut command| {
        command
            .status()
            .with_context(|| "failed to wait for child command")
    }) {
        Ok(status) => status.code().unwrap_or(1).clamp(0, 255) as u8,
        Err(error) => {
            eprintln!("rterm: failed to spawn `{}`: {error}", command[0]);
            127
        }
    };

    if let Ok(exit_file) = std::env::var("RTERM_EXIT_FILE") {
        let _ = std::fs::write(exit_file, code.to_string());
    }
    print!("\x1b]6973;rterm-exit:{marker}:{code}\x07");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    ExitCode::from(code)
}

async fn run() -> anyhow::Result<u8> {
    let cli = Cli::parse();
    let token = cli.token.clone().unwrap_or_else(token::generate);
    let bind_addr = cli.effective_bind();
    let word_erase = cli.decoded_word_erase();

    print_startup(&cli, bind_addr, &token);

    let config = RunConfig {
        command: cli.command,
        bind_addr,
        lan: cli.lan,
        web_write: cli.write,
        max_clients: cli.max_clients,
        once: cli.once,
        headless: cli.headless,
        token,
        word_erase,
    };

    run_session(config).await.context("terminal session failed")
}

fn print_startup(cli: &Cli, bind_addr: std::net::SocketAddr, token: &str) {
    let local_url = format!("http://127.0.0.1:{}/t/{token}", bind_addr.port());
    let lan_url = lan_ip::primary_lan_ip()
        .map(|ip| format!("http://{ip}:{}/t/{token}", bind_addr.port()))
        .unwrap_or_else(|| "(no LAN address detected)".to_string());
    let mode = if cli.write { "writable" } else { "read-only" };

    eprintln!("rterm");
    eprintln!("  Local URL: {local_url}");
    if cli.lan {
        eprintln!("  LAN URL:   {lan_url}");
        eprintln!(
            "  Warning: LAN mode exposes this terminal URL to devices on your local network."
        );
    }
    eprintln!("  Web mode:  {mode}, max {} client(s)", cli.max_clients);
    eprintln!("  Bind:      {bind_addr}");

    if wsl::is_wsl() && cli.lan {
        eprintln!();
        eprintln!("{}", wsl::lan_guidance(bind_addr.port()));
    }
}

fn init_tracing() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|error| anyhow::anyhow!("tracing subscriber already initialized: {error}"))
}
