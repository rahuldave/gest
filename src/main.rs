mod actions;
mod cli;
mod config;
mod io;
mod logging;
mod store;
mod ui;
mod web;

use std::{path::PathBuf, process::ExitCode, sync::Arc};

use clap::Parser;
use getset::Getters;

use crate::{
  cli::{App, Error},
  config::Settings,
  logging::LevelFilter,
  store::{Db, model::primitives::Id},
  ui::{components::ErrorMessage, style, style::Theme},
};

/// Shared application state threaded through every subcommand.
#[derive(Clone, Debug, Getters)]
pub struct AppContext {
  /// The `.gest` directory path, if the project is in local mode.
  #[get = "pub"]
  gest_dir: Option<PathBuf>,
  /// Whether the user passed `--no-pager` to suppress paging of long command output.
  #[get = "pub"]
  no_pager: bool,
  /// The current project's ID, if one has been initialized for the working directory.
  #[get = "pub"]
  project_id: Option<Id>,
  /// Resolved configuration settings (file + env overrides).
  #[get = "pub"]
  settings: Settings,
  /// Database connection handle.
  #[get = "pub"]
  store: Arc<Db>,
}

#[tokio::main]
async fn main() -> ExitCode {
  match run().await {
    Ok(()) => ExitCode::SUCCESS,
    Err(err) => {
      eprintln!("{}", ErrorMessage::new(err.to_string()));
      err.exit_code()
    }
  }
}

/// Drive the CLI from argv to result. All error paths funnel through `cli::Error`
/// so [`main`] can compute a sysexits.h-compliant exit code via
/// [`cli::Error::exit_code`].
async fn run() -> Result<(), Error> {
  ui::init();

  let app = App::parse();
  let verbosity = match app.verbosity_level() {
    1 => Some(LevelFilter::Info),
    2 => Some(LevelFilter::Debug),
    3 => Some(LevelFilter::Trace),
    _ => None,
  };

  logging::init(verbosity.unwrap_or(LevelFilter::default()));

  let settings = config::load()?;
  if verbosity.is_none() {
    log::set_max_level(settings.log().level().into());
  }
  log::info!("config loaded");

  style::set_global(Theme::from_config(&settings));

  let startup_gest_dir = prepare_startup_gest_dir(&app)?;
  let store = store::open_with_gest_dir(&settings, startup_gest_dir.as_deref()).await?;
  log::info!("store opened");
  let (project_id, gest_dir) = resolve_project(&store).await?;

  // Configure transparent sync in the store layer
  if settings.storage().sync_enabled()
    && let (Some(pid), Some(dir)) = (&project_id, &gest_dir)
  {
    store.configure_sync(pid.clone(), dir.clone());
  }

  store.import_if_needed().await?;

  let context = AppContext {
    gest_dir: gest_dir.clone(),
    no_pager: *app.no_pager(),
    project_id,
    settings,
    store: store.clone(),
  };

  log::info!("command dispatched");
  app.call(&context).await?;

  store.export_if_needed().await?;
  Ok(())
}

/// Resolve the current project ID and `.gest` directory from the working directory.
///
/// Returns `(None, None)` when the current directory is not itself registered
/// as a gest project (the common case for bootstrapping commands like `init`).
async fn resolve_project(store: &Arc<Db>) -> Result<(Option<Id>, Option<PathBuf>), Error> {
  let Ok(cwd) = std::env::current_dir() else {
    return Ok((None, None));
  };
  let conn = store.connect().await?;
  match store::repo::project::find_by_path(&conn, &cwd).await? {
    Some(project) => {
      let gest_dir = store::sync::find_gest_dir(project.root());
      Ok((Some(project.id().clone()), gest_dir))
    }
    None => {
      let Some(gest_dir) = find_ancestor_gest_dir(&cwd) else {
        return Ok((None, None));
      };
      let Some(root) = gest_dir.parent() else {
        return Ok((None, None));
      };
      match store::repo::project::find_or_create_from_synced_project(&conn, root, &gest_dir).await? {
        Some(project) => Ok((Some(project.id().clone()), Some(gest_dir))),
        None => Ok((None, None)),
      }
    }
  }
}

/// Discover or create the `.gest` directory before the store opens.
///
/// `init --local` is special: it must create `.gest` early so the first store
/// open can use `.gest/gest.db` instead of briefly touching the global data
/// directory.
fn prepare_startup_gest_dir(app: &App) -> Result<Option<PathBuf>, Error> {
  let Ok(cwd) = std::env::current_dir() else {
    return Ok(None);
  };

  if app.initializes_local_project() {
    let dir = cwd.join(".gest");
    std::fs::create_dir_all(&dir)?;
    return Ok(Some(dir));
  }

  Ok(find_ancestor_gest_dir(&cwd))
}

/// Walk from `start` upward looking for a project `.gest` directory.
fn find_ancestor_gest_dir(start: &std::path::Path) -> Option<PathBuf> {
  let mut current = start;
  loop {
    let candidate = current.join(".gest");
    if candidate.is_dir() {
      return Some(candidate);
    }
    current = current.parent()?;
  }
}
