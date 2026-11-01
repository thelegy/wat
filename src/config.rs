use std::{collections::HashMap, path::Path};

use anyhow::Result;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Connection {
    #[default]
    Ssh,
    Sudo,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    connection: Connection,
    hosts: HashMap<String, HostConfig>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct HostConfig {
    connection: Option<Connection>,
}

impl Config {
    pub fn load(directory: &Path) -> Result<Self> {
        let path = directory.join(".wat/config");
        config::Config::builder()
            .add_source(config::File::from(path.as_path()).required(false))
            .build()
            .and_then(config::Config::try_deserialize)
            .map_err(|error| {
                anyhow::anyhow!(
                    "failed to load configuration from {}: {error}",
                    path.display()
                )
            })
    }

    pub fn connection_for(&self, hostname: &str) -> Connection {
        self.hosts
            .get(hostname)
            .and_then(|host| host.connection)
            .unwrap_or(self.connection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(contents: &str, extension: &str) -> Result<Config> {
        let directory = tempfile::tempdir()?;
        std::fs::create_dir(directory.path().join(".wat"))?;
        std::fs::write(
            directory.path().join(format!(".wat/config.{extension}")),
            contents,
        )?;
        Config::load(directory.path())
    }

    #[test]
    fn missing_config_defaults_to_ssh() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            Config::load(directory.path())
                .unwrap()
                .connection_for("local"),
            Connection::Ssh
        );
    }

    #[test]
    fn toml_host_overrides_and_inheritance() {
        let config = load(
            "connection = 'sudo'\n[hosts.remote]\nconnection = 'ssh'\n[hosts.local]\n",
            "toml",
        )
        .unwrap();
        assert_eq!(config.connection_for("remote"), Connection::Ssh);
        assert_eq!(config.connection_for("local"), Connection::Sudo);
        assert_eq!(config.connection_for("other"), Connection::Sudo);
        let config = load("[hosts.local]\nconnection = 'sudo'", "toml").unwrap();
        assert_eq!(config.connection_for("local"), Connection::Sudo);
        assert_eq!(config.connection_for("other"), Connection::Ssh);
    }

    #[test]
    fn json_and_yaml_are_supported() {
        for (contents, extension) in [
            (r#"{"connection":"sudo"}"#, "json"),
            ("connection: sudo", "yaml"),
        ] {
            assert_eq!(
                load(contents, extension).unwrap().connection_for("local"),
                Connection::Sudo
            );
        }
    }

    #[test]
    fn invalid_config_is_rejected() {
        for contents in [
            "connection = 'invalid'",
            "connection = [",
            "conection = 'sudo'",
            "[hosts.local]\nconection = 'sudo'",
        ] {
            assert!(load(contents, "toml").is_err(), "{contents}");
        }
    }
}
