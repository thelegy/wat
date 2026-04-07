use std::{path::Path, process::Command};

use anyhow::{Context, Result, bail};

use super::{Backend, LogFormat};

pub struct LixBackend {
    binary: String,
}

impl LixBackend {
    pub fn new(binary: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    fn binary(&self) -> &str {
        &self.binary
    }

    fn command(&self) -> Command {
        Command::new(self.binary())
    }
}

impl Backend for LixBackend {
    fn build_installable(
        &self,
        installable: &str,
        out_link: &Path,
        keep_going: bool,
        log_format: LogFormat,
    ) -> Result<()> {
        let mut command = self.command();
        command.args(["--log-format", log_format.as_arg(), "build"]);
        if keep_going {
            command.arg("--keep-going");
        }
        command.arg(installable);
        command.arg("--out-link");
        command.arg(out_link);
        execute_command(command, self.binary())
    }

    fn copy_closure(
        &self,
        source: &Path,
        destination: &str,
        substitute_on_destination: bool,
        log_format: LogFormat,
    ) -> Result<()> {
        let mut command = self.command();
        command.args(["--log-format", log_format.as_arg(), "copy"]);
        if substitute_on_destination {
            command.arg("--substitute-on-destination");
        }
        command.arg("--to");
        command.arg(destination);
        command.arg(source);
        execute_command(command, self.binary())
    }

    fn list_machines(&self) -> Result<Vec<String>> {
        let output = self
            .command()
            .args([
                "eval",
                "--json",
                "--apply",
                "builtins.attrNames",
                ".#nixosConfigurations",
            ])
            .output()
            .with_context(|| {
                format!("failed to invoke {} for hostname completion", self.binary())
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "{} exited with status {}: {}",
                self.binary(),
                output.status,
                stderr.trim()
            );
        }

        let stdout =
            String::from_utf8(output.stdout).context("hostname output contained invalid UTF-8")?;
        let mut hostnames: Vec<String> =
            serde_json::from_str(&stdout).context("failed to parse hostname list")?;
        hostnames.sort();
        Ok(hostnames)
    }
}

fn execute_command(mut command: Command, binary: &str) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("failed to execute {binary}"))?;
    if status.success() {
        Ok(())
    } else {
        bail!("{binary} exited with status {status}");
    }
}
