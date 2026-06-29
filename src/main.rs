use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;
use rterm::cli::{Cli, CliCommand, RunArgs};
use rterm::platform::command;
use rterm::platform::{ctrl_c, elevation, lan_ip, wsl};
use rterm::security::token;
use rterm::session::{RunConfig, registry, run_session};
use rterm::web::server;
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
    if let Err(error) = ctrl_c::protect_child_helper() {
        eprintln!("rterm: failed to protect child helper from Ctrl+C: {error:#}");
        return ExitCode::from(1);
    }

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
    if let Some(command) = cli.subcommand {
        return run_cli_command(command);
    }

    let bind_addr = cli.effective_bind();
    let word_erase = cli.decoded_word_erase()?;
    let backspace = cli.decoded_backspace()?;
    let run = cli.run;
    anyhow::ensure!(
        !run.command.is_empty(),
        "a child command is required after `--`"
    );
    elevation::ensure_session_allowed(run.allow_elevated)?;
    if let Some(value) = &run.token {
        token::validate_user_supplied(value)?;
    }
    let token = run.token.clone().unwrap_or_else(token::generate);

    let listener = server::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind web server at {bind_addr}"))?;
    let bind_addr = listener
        .local_addr()
        .context("failed to determine the web server listener address")?;
    print_startup(&run, bind_addr, &token);

    let config = RunConfig {
        command: run.command,
        bind_addr,
        lan: run.lan,
        web_write: run.write,
        max_clients: run.max_clients,
        once: run.once,
        headless: run.headless,
        token,
        backspace,
        word_erase,
    };

    run_session(config, listener)
        .await
        .context("terminal session failed")
}

fn run_cli_command(command: CliCommand) -> anyhow::Result<u8> {
    match command {
        CliCommand::Sessions(args) => {
            let sessions = registry::list()?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&sessions)?);
            } else if sessions.is_empty() {
                println!("No active rterm sessions.");
            } else {
                for session in sessions {
                    let mode = if session.writable {
                        "writable"
                    } else {
                        "read-only"
                    };
                    println!(
                        "{}  pid={}  {}  {}",
                        session.id, session.pid, mode, session.program
                    );
                    println!("  Local URL: {}", session.local_url);
                    if let Some(url) = session.lan_url {
                        println!("  LAN URL:   {url}");
                    }
                }
            }
            Ok(0)
        }
    }
}

fn print_startup(cli: &RunArgs, bind_addr: std::net::SocketAddr, token: &str) {
    let local_url = lan_ip::terminal_url(
        lan_ip::local_access_ip(bind_addr.ip()),
        bind_addr.port(),
        token,
    );
    let lan_url = lan_ip::primary_lan_ip()
        .map(|ip| lan_ip::terminal_url(ip, bind_addr.port(), token))
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
