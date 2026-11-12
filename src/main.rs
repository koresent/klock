use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use smithay_client_toolkit::reexports::calloop::EventLoop;
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use wayland_client::Connection;
use wayland_client::globals::registry_queue_init;

mod auth;
mod config;
mod render;
mod wayland;

use config::Config;
use wayland::LockApp;

#[derive(Parser, Debug)]
#[command(
    name = "klock",
    version = "0.1.0",
    author = "Koresent <koresent@pm.me>",
    about = "Wayland session lockscreen matching kgreet styling"
)]
struct CliArgs {
    #[arg(short, long, help = "Path to custom YAML configuration file")]
    config: Option<PathBuf>,

    #[arg(
        long,
        help = "Run in a standard desktop window for UI preview and testing"
    )]
    windowed: bool,

    #[arg(
        long,
        help = "Mock PAM authentication (password 'test' or empty unlocks)"
    )]
    mock_pam: bool,

    #[arg(
        long,
        help = "Allow pressing Escape to immediately unlock (development only)"
    )]
    dev_escape: bool,

    #[arg(long, help = "Target monitor name for primary login card (e.g. DP-2)")]
    target: Option<String>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = CliArgs::parse();
    let cfg = Config::load(args.config.as_deref())?;

    let conn = Connection::connect_to_env().context("Failed to connect to Wayland compositor")?;

    let (globals, event_queue) =
        registry_queue_init(&conn).context("Failed to initialize Wayland registry queue")?;
    let qh = event_queue.handle();

    let mut event_loop: EventLoop<LockApp> =
        EventLoop::try_new().context("Failed to create calloop event loop")?;

    WaylandSource::new(conn.clone(), event_queue)
        .insert(event_loop.handle())
        .map_err(|e| anyhow::anyhow!("Failed to register Wayland source: {:?}", e))?;

    let mut app = LockApp::new(
        conn,
        event_loop.handle(),
        cfg,
        args.windowed,
        args.mock_pam,
        args.dev_escape,
        args.target,
        &qh,
        &globals,
    )?;

    app.start(&qh)?;

    while !app.exit {
        event_loop
            .dispatch(Duration::from_millis(16), &mut app)
            .context("Error dispatching event loop")?;
    }

    Ok(())
}
