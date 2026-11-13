//! `gest self-update`: download and install the latest release from GitHub.

use clap::Args;
use self_update::backends::github::Update;

use crate::{AppContext, cli::Error, ui::components::SuccessMessage};

/// Download and install the latest release from GitHub.
#[derive(Args, Debug)]
pub struct Command {
  /// Pin to a specific release version (e.g. `0.5.0`).
  #[arg(long)]
  target: Option<String>,
}

impl Command {
  /// Check for updates and, if a newer version is available, download and
  /// replace the current binary.
  ///
  /// # Arguments
  ///
  /// * `_context` - the application context, which this command does not use
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, _context: &AppContext) -> Result<(), Error> {
    let current_version = env!("CARGO_PKG_VERSION");
    let target = self.target.clone();

    let status = tokio::task::spawn_blocking(move || {
      let mut builder = Update::configure();
      builder
        .repo_owner("aaronmallen")
        .repo_name("gest")
        .bin_name("gest")
        .asset_identifier("tar.gz")
        .current_version(current_version)
        .show_download_progress(true)
        .no_confirm(true);

      if let Some(ref version) = target {
        builder.release_tag(format!("v{version}"));
      }

      let updater = builder.build()?;

      let target_version = match target.as_deref() {
        Some(version) => version.to_string(),
        None => updater
          .get_latest_release()?
          .latest()
          .map(|release| release.version().to_string())
          .ok_or_else(self_update::Error::no_release_found)?,
      };

      if target_version != current_version {
        let parts: Vec<&str> = target_version.splitn(3, '.').collect();
        let version_path = format!("{}.{}", parts[0], parts.get(1).unwrap_or(&"0"));
        println!(
          "review the changelog: https://gest.aaronmallen.dev/docs/{version_path}/changelog#{}",
          changelog_anchor(&target_version)
        );
      }

      updater.update()
    })
    .await
    .map_err(std::io::Error::other)?
    .map_err(std::io::Error::other)?;

    if status.is_updated() {
      let message = SuccessMessage::new("updated gest")
        .field("previous version", current_version)
        .field("new version", status.version());
      println!("{message}");
    } else {
      let message = SuccessMessage::new("gest is already up to date").field("version", current_version);
      println!("{message}");
    }

    Ok(())
  }
}

/// Build the changelog anchor fragment for a given semver version (e.g. `0.5.1` → `v051`).
///
/// # Arguments
///
/// * `version` - the version, without a leading `v`
///
/// # Returns
///
/// `v` followed by the version with its dots removed.
fn changelog_anchor(version: &str) -> String {
  format!("v{}", version.replace('.', ""))
}

#[cfg(test)]
mod tests {
  use super::*;

  mod progress_bar {
    use super::*;

    /// self_update 1.x draws the bar from `show_download_progress` only with its `progress-bar` feature, and
    /// Gest turns its default features off. `ProgressStyle` and `progress_style` exist only with that feature,
    /// so this test does not compile without it.
    #[test]
    fn it_builds_self_update_with_the_download_progress_bar() {
      let mut builder = Update::configure();
      builder.progress_style(self_update::ProgressStyle::new(
        "[{bar:40}] {bytes}/{total_bytes}",
        "=>-",
      ));
    }
  }

  mod changelog_anchor {
    use super::*;

    #[test]
    fn it_strips_dots_and_prefixes_v() {
      assert_eq!(changelog_anchor("0.5.1"), "v051");
      assert_eq!(changelog_anchor("1.0.0"), "v100");
    }
  }
}
