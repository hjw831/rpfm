//---------------------------------------------------------------------------//
// Copyright (c) 2017-2026 Ismael Gutiérrez González. All rights reserved.
//
// This file is part of the Rusted PackFile Manager (RPFM) project,
// which can be found here: https://github.com/Frodo45127/rpfm.
//
// This file is licensed under the MIT license, which can be found here:
// https://github.com/Frodo45127/rpfm/blob/master/LICENSE.
//---------------------------------------------------------------------------//

//! # `rpfm_server`
//!
//! Backend process for [Rusted PackFile Manager][rpfm]. Hosts the heavy work
//! that the Qt6 UI ([`rpfm_ui`][ui]) and AI / MCP clients drive remotely:
//! Pack I/O, schema decoding, diagnostics, search, dependencies, optimisation
//! and so on.
//!
//! [rpfm]: https://github.com/Frodo45127/rpfm
//! [ui]: https://crates.io/crates/rpfm_ui
//!
//! ## Architecture
//!
//! The server is built on [`axum`] (HTTP + WebSocket) and [`tokio`]. It binds
//! to `127.0.0.1:45127` by default and exposes three endpoints:
//!
//! | Endpoint    | Method | Purpose                                                                          |
//! |-------------|--------|----------------------------------------------------------------------------------|
//! | `/ws`       | GET    | WebSocket upgrade. Carries the [`rpfm_ipc`] command/response protocol.           |
//! | `/version`  | GET    | REST: report the server build version + pid.    |
//! | `/sessions` | GET    | REST: list every active session (used by the UI session picker).                 |
//! | `/mcp`      | *      | MCP `StreamableHttpService` exposing the same surface to AI / MCP clients.       |
//!
//! Every client connection is wrapped in a [`session::Session`] managed by a
//! [`session::SessionManager`]. Each session owns a dedicated background
//! thread (see [`background_thread`]) that processes commands serially against
//! its own in-memory state (open packs, dependency cache, settings cache),
//! so multiple concurrent clients can't step on each other.
//!
//! ## Modules
//!
//! - [`background_thread`] — central command dispatcher; one async loop per session.
//! - [`comms`] — generic mpsc-based request/response abstraction used to talk
//!   to the background thread.
//! - [`server_websocket`] — `/ws` upgrade handler and message multiplexer.
//! - [`server_mcp`] — `/mcp` endpoint: tools, prompts, resources for MCP clients.
//! - [`session`] — `SessionManager`, `Session`, lifecycle and timeout handling.
//! - [`settings`] — JSON-backed settings store with batch-write optimisation.
//! - [`updater`] — self-update checks against GitHub releases.
//!
//! ## Telemetry
//!
//! Logging, panic capture and action telemetry are wired through
//! [`rpfm_telemetry`]. The Sentry guard returned by [`Logger::init`] is held
//! for the process lifetime in [`main`].

// Under windows, hide the server window by default.
#![windows_subsystem = "windows"]

use axum::{extract::State, routing::get, Json, Router};
use rmcp::transport::streamable_http_server::{session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use rmcp::ServiceExt;

use rpfm_ipc::helpers::SessionInfo;
use rpfm_ipc::settings_keys::{ANONYMOUS_TELEMETRY_ID, ENABLE_CRASH_REPORTS, ENABLE_USAGE_TELEMETRY};

use rpfm_telemetry::{Logger, SentryLayer, SENTRY_DSN, info, release_name, warn};

use crate::server_mcp::McpServer;
use crate::session::SessionManager;
use rpfm_ipc::settings::{error_path, init_config_path, Settings};
use crate::server_websocket::ws_handler;

pub mod api;
pub mod background_thread;
pub mod ceo_builder;
pub mod jobs;
pub mod server_mcp;
pub mod server_websocket;
pub mod session;
pub mod state;
pub mod translation_hub;
pub mod updater;
#[cfg(test)] mod updater_test;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

//-------------------------------------------------------------------------------//
//                                  Constants
//-------------------------------------------------------------------------------//

/// Sentry DSN used for crash reports.
const SENTRY_DSN_KEY: &str = match option_env!("RPFM_SERVER_SENTRY_DSN") {
    Some(dsn) => dsn,
    None => "",
};

/// PostHog project API key used for telemetry and feedback.
const POSTHOG_API_KEY_VALUE: &str = match option_env!("RPFM_SERVER_POSTHOG_API_KEY") {
    Some(key) => key,
    None => "",
};

/// Client ID of RPFM's GitHub OAuth App, used to sign in to submit translations.
pub const GITHUB_OAUTH_CLIENT_ID: &str = match option_env!("RPFM_SERVER_GITHUB_OAUTH_CLIENT_ID") {
    Some(id) => id,
    None => "",
};

/// Default IP address the HTTP server binds to (`127.0.0.1` / loopback).
const DEFAULT_ADDRESS: [u8; 4] = [127, 0, 0, 1];

/// Default TCP port the HTTP server listens on.
const DEFAULT_PORT: u16 = 45127;

//-------------------------------------------------------------------------------//
//                                  Functions
//-------------------------------------------------------------------------------//

/// Process entry point.
#[tokio::main]
async fn main() {

    // Sentry client guard, so we can reuse it later on and keep it in scope for the entire duration of the program.
    *SENTRY_DSN.write().unwrap() = SENTRY_DSN_KEY.to_owned();
    rpfm_telemetry::set_posthog_api_key(POSTHOG_API_KEY_VALUE);
    let guard = Logger::init(&{
        init_config_path().expect("Error while trying to initialize config path. We're fucked.");
        error_path().unwrap_or_else(|_| PathBuf::from("."))
    }, true, false, release_name!()).expect("Failed to initialize logging system.");

    // Setup tracing subscriber for logging, redirecting to stderr to avoid interfering with MCP.
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_filter(tracing_subscriber::filter::LevelFilter::INFO))
        .with(SentryLayer::default())
        .init();

    if guard.is_enabled() {
        info!("Sentry logging support for RPFM SERVER enabled. Starting...");
    } else {
        info!("Sentry logging support for RPFM SERVER disabled. Starting...");
    }

    apply_telemetry_settings(&Settings::init(false));

    rpfm_telemetry::set_event_property("release", serde_json::Value::from(env!("CARGO_PKG_VERSION")));
    rpfm_telemetry::set_event_property("os", serde_json::Value::from(std::env::consts::OS));
    rpfm_telemetry::set_event_property("is_beta", serde_json::Value::from(rpfm_telemetry::is_beta()));

    let session_manager: Arc<SessionManager> = Arc::new(SessionManager::default());
    SessionManager::start_cleanup_task(session_manager.clone());

    // Check for MCP Stdio mode.
    let args: Vec<String> = env::args().collect();
    if args.contains(&"--mcp-stdio".to_string()) {
        info!("Starting RPFM Server in MCP Stdio mode...");
        let mcp_server = McpServer::new(session_manager.clone());

        // Use the official rmcp Stdio transport
        match mcp_server.serve(rmcp::transport::stdio()).await {
            Ok(_) => info!("MCP Stdio server finished normally."),
            Err(e) => warn!("MCP Stdio server encountered an error: {e}"),
        }
        return;
    }

    // Create the MCP service. All MCP clients share one session, got from the manager on each call.
    let sm = session_manager.clone();
    let http_service = StreamableHttpService::new(
        move || Ok(McpServer::new(sm.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default().with_allowed_origins(allowed_origins()),
    );

    // Setup the endpoints for the server.
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/version", get(version_handler))
        .route("/sessions", get(sessions_handler))
        .nest_service("/mcp", http_service)
        .with_state(session_manager);

    let addr = SocketAddr::from((DEFAULT_ADDRESS, DEFAULT_PORT));
    match TcpListener::bind(addr).await {
        Ok(listener) => {
            info!("Listening on {}", addr);
            axum::serve(listener, app).await.unwrap();
        }
        Err(err) => {
            warn!("Failed to bind to address {}: {}\n\nThis usually means you got another copy of the server running. Either use that one, or stop it and try again.", addr, err);
        }
    }
}

pub fn allowed_origins() -> Vec<String> {
    ["localhost", "127.0.0.1", "[::1]"].iter()
        .map(|host| format!("http://{host}:{DEFAULT_PORT}"))
        .collect()
}

pub fn apply_telemetry_settings(settings: &Settings) {
    rpfm_telemetry::set_usage_telemetry_enabled(settings.bool(ENABLE_USAGE_TELEMETRY));
}

async fn version_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "pid": std::process::id(),
    }))
}

async fn sessions_handler(State(session_manager): State<Arc<SessionManager>>) -> Json<Vec<SessionInfo>> {
    let sessions = session_manager.get_sessions_info();
    info!("Sessions endpoint queried: {} active session(s)", sessions.len());
    Json(sessions)
}
