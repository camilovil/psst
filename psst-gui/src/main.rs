#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::new_without_default, clippy::type_complexity)]

mod cmd;
mod controller;
mod data;
mod delegate;
mod error;
mod ui;
mod webapi;
mod widget;

use std::{
    fs::{self, File},
    io::{self, Write},
};

use druid::AppLauncher;
use env_logger::{Builder, Env, Target};
use webapi::WebApi;

use psst_core::cache::Cache;

use crate::{
    data::{AppState, Config},
    delegate::Delegate,
};

const ENV_LOG: &str = "PSST_LOG";
const ENV_LOG_STYLE: &str = "PSST_LOG_STYLE";

fn main() {
    // Setup logging from the env variables, with defaults.
    let mut logger = Builder::from_env(
        Env::new()
            .filter_or(ENV_LOG, "info")
            .write_style(ENV_LOG_STYLE),
    );
    if let Some(file) = open_log_file() {
        logger.target(Target::Pipe(Box::new(Tee { file })));
    }
    logger.init();

    // Panics on the UI thread abort the process (they can't unwind out of the
    // window procedure), so record them in the log before that happens.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("panic: {info}
{backtrace}");
        default_hook(info);
    }));

    // Load configuration
    let config = Config::load().unwrap_or_default();

    let paginated_limit = config.paginated_limit;
    let mut state = AppState::default_with_config(config.clone());

    if let Some(cache_dir) = Config::cache_dir() {
        match Cache::new(cache_dir) {
            Ok(cache) => {
                state.preferences.cache = Some(cache);
            }
            Err(err) => {
                log::error!("Failed to create cache: {err}");
            }
        }
    }

    WebApi::new(
        Config::proxy().as_deref(),
        Config::cache_dir(),
        paginated_limit,
    )
    .install_as_global();

    // Share the core session so the WebApi can authenticate `api-partner`
    // (pathfinder GraphQL) calls with first-party tokens.
    WebApi::global().set_session(state.session.clone());

    let delegate;
    let launcher;
    if state.config.has_credentials() {
        // Credentials are configured, open the main window.

        // Seed the WebApi with the stored token even if it has expired: it is
        // refreshed on first use from a worker thread, so the window doesn't
        // wait on a network round-trip. Without a token, the user can
        // re-authenticate from preferences.
        if state.config.stored_webapi_token().is_none() {
            log::warn!("No Web API token, re-authentication required");
        }
        WebApi::global().set_webapi_credentials(
            state.config.webapi_client_id_value().map(String::from),
            state.config.stored_webapi_token().cloned(),
        );

        let window = ui::main_window(&state.config);
        delegate = Delegate::with_main(window.id);
        launcher = AppLauncher::with_window(window).configure_env(ui::theme::setup);

        // Load user's local tracks for the WebApi.
        WebApi::global().load_local_tracks(state.config.username().unwrap());
    } else {
        // No configured credentials, open the account setup.
        let window = ui::account_setup_window();
        delegate = Delegate::with_preferences(window.id);
        launcher = AppLauncher::with_window(window).configure_env(ui::theme::setup);
    };

    // Refreshed Web API tokens are persisted by the delegate, which owns the
    // config.
    WebApi::global().set_token_sink(launcher.get_external_handle());

    launcher
        .delegate(delegate)
        .launch(state)
        .expect("Application launch");
}

/// Release builds have no console on Windows, so also keep the log in a file
/// next to the cache, to be able to tell what went wrong afterwards.
fn open_log_file() -> Option<File> {
    const MAX_LOG_SIZE: u64 = 5 * 1024 * 1024;

    let dir = Config::cache_dir()?;
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join("psst.log");
    // Keep one previous log around instead of growing without bound.
    if fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_LOG_SIZE) {
        let _ = fs::rename(&path, dir.join("psst.old.log"));
    }
    File::options().create(true).append(true).open(path).ok()
}

/// Writes log output both to stderr and to the log file.
struct Tee {
    file: File,
}

impl Write for Tee {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let _ = io::stderr().write_all(buf);
        self.file.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let _ = io::stderr().flush();
        self.file.flush()
    }
}
