//! Git configuration utilities.

use std::process::Command;

/// Author identity resolved from git config.
pub struct GitAuthor {
  /// Email address from `user.email`, or `None` when unset.
  pub email: Option<String>,
  /// Display name from `user.name` or the `$USER` fallback.
  pub name: String,
}

/// Resolve the current git user identity from `git config`.
///
/// Reads `user.name` and `user.email` from git config. Returns `None` if
/// `user.name` is not configured. `email` is optional and may be `None`
/// even when `name` is present.
///
/// # Returns
///
/// The author; `None` also when `git` cannot run.
pub fn resolve_author() -> Option<GitAuthor> {
  log::debug!("git: resolving author from git config");
  let name = git_config("user.name")?;
  let email = git_config("user.email");
  log::trace!(
    "git: resolved name={name} email={}",
    email.as_deref().unwrap_or("<none>")
  );
  Some(GitAuthor {
    name,
    email,
  })
}

/// Resolve the current user identity, trying git config first, then `$USER`.
///
/// # Returns
///
/// The author from git config, else one named by `$USER` with no email.
/// Returns `None` only if both git config and `$USER` are unavailable.
pub fn resolve_author_or_env() -> Option<GitAuthor> {
  log::debug!("git: resolving author (git config or $USER fallback)");
  resolve_author().or_else(|| {
    std::env::var("USER").ok().map(|name| {
      log::trace!("git: falling back to $USER={name}");
      GitAuthor {
        name,
        email: None,
      }
    })
  })
}

/// Read one value with `git config <key>`.
///
/// # Arguments
///
/// * `key` - the config key, such as `user.name`
///
/// # Returns
///
/// The trimmed value, or `None` when the key is unset, empty, or `git` cannot run.
fn git_config(key: &str) -> Option<String> {
  log::trace!("git: reading config key {key}");
  let output = Command::new("git").args(["config", key]).output().ok()?;
  if !output.status.success() {
    log::trace!("git: config key {key} not set");
    return None;
  }
  let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
  if value.is_empty() { None } else { Some(value) }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod git_config_fn {
    use super::*;

    #[test]
    fn it_returns_none_for_nonexistent_key() {
      assert!(git_config("gest.nonexistent.test.key").is_none());
    }
  }
}
