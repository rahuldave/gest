use getset::Getters;
use serde::{Deserialize, Serialize};

/// Database connection settings from the `[database]` configuration table.
///
/// Connection details can be provided either as a complete URL or as individual
/// components (scheme, host, port, username, password). When both a URL and
/// individual components are present, the explicit URL takes precedence.
#[derive(Clone, Debug, Default, Deserialize, Getters, PartialEq, Serialize)]
#[serde(default)]
pub struct Settings {
  /// Bearer or API token for database authentication.
  #[get = "pub"]
  auth_token: Option<String>,
  /// Database server hostname or IP address.
  #[get = "pub"]
  host: Option<String>,
  /// Password for username/password authentication.
  #[get = "pub"]
  password: Option<String>,
  /// Port number the database server listens on.
  #[get = "pub"]
  port: Option<u16>,
  /// Connection scheme (e.g. `sqlite`, `postgres`). Defaults to `sqlite` when building a URL
  /// from components.
  #[get = "pub"]
  scheme: Option<String>,
  /// Explicit connection URL. When set, component fields are ignored by [`Settings::url`].
  url: Option<String>,
  /// Username for database authentication.
  #[get = "pub"]
  username: Option<String>,
}

impl Settings {
  /// Returns the database connection URL.
  ///
  /// If an explicit `url` was configured, it is returned as-is. Otherwise a URL is assembled
  /// from the individual component fields in the form `scheme://[user[:pass]@]host[:port]`.
  ///
  /// # Returns
  ///
  /// The connection URL. Returns `None` when no `url` or `host` is configured.
  pub fn url(&self) -> Option<String> {
    if let Some(url) = &self.url {
      log::debug!("database.url is set to: {url}");
      log::trace!("using database.url");
      return Some(url.clone());
    }

    let host = self.host.as_deref()?;
    let scheme = self.scheme.as_deref().unwrap_or("sqlite");
    let mut url = format!("{scheme}://");

    if let Some(username) = &self.username {
      url.push_str(username);
      if let Some(password) = &self.password {
        url.push(':');
        url.push_str(password);
      }
      url.push('@');
    }

    url.push_str(host);

    if let Some(port) = self.port {
      url.push(':');
      url.push_str(&port.to_string());
    }

    log::debug!("database url resolves to {url}");
    log::trace!("using {url}");
    Some(url)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod url {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_builds_full_url_from_all_components() {
      let settings = Settings {
        scheme: Some("libsql".into()),
        host: Some("db.example.com".into()),
        port: Some(8080),
        username: Some("user".into()),
        password: Some("pass".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("libsql://user:pass@db.example.com:8080".into()));
    }

    #[test]
    fn it_builds_url_from_host_with_default_scheme() {
      let settings = Settings {
        host: Some("localhost".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("sqlite://localhost".into()));
    }

    #[test]
    fn it_builds_url_with_custom_scheme() {
      let settings = Settings {
        scheme: Some("libsql".into()),
        host: Some("db.example.com".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("libsql://db.example.com".into()));
    }

    #[test]
    fn it_ignores_password_without_username() {
      let settings = Settings {
        host: Some("localhost".into()),
        password: Some("secret".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("sqlite://localhost".into()));
    }

    #[test]
    fn it_includes_port() {
      let settings = Settings {
        host: Some("localhost".into()),
        port: Some(5432),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("sqlite://localhost:5432".into()));
    }

    #[test]
    fn it_includes_username() {
      let settings = Settings {
        host: Some("localhost".into()),
        username: Some("admin".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("sqlite://admin@localhost".into()));
    }

    #[test]
    fn it_includes_username_and_password() {
      let settings = Settings {
        host: Some("localhost".into()),
        username: Some("admin".into()),
        password: Some("secret".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("sqlite://admin:secret@localhost".into()));
    }

    #[test]
    fn it_returns_explicit_url_as_is() {
      let settings = Settings {
        url: Some("libsql://my-db.turso.io".into()),
        host: Some("ignored.example.com".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("libsql://my-db.turso.io".into()));
    }

    #[test]
    fn it_returns_none_when_no_url_or_host() {
      let settings = Settings::default();

      assert_eq!(settings.url(), None);
    }
  }
}
