use std::{
    path::Path,
    process::Stdio,
};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

use super::{Backend, lix_renderer::LixRenderer};

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

#[async_trait]
impl Backend for LixBackend {
    async fn build_installable(
        &self,
        installable: &str,
        out_link: &Path,
        keep_going: bool,
    ) -> Result<()> {
        let mut command = self.command();
        command.args(["--log-format", "internal-json", "build"]);
        if keep_going {
            command.arg("--keep-going");
        }
        command.arg(installable);
        command.arg("--out-link");
        command.arg(out_link);
        execute_command(&mut command).await
    }

    async fn copy_closure(
        &self,
        source: &Path,
        destination: &str,
        substitute_on_destination: bool,
    ) -> Result<()> {
        let mut command = self.command();
        command.args(["--log-format", "internal-json", "copy"]);
        if substitute_on_destination {
            command.arg("--substitute-on-destination");
        }
        command.arg("--to");
        command.arg(destination);
        command.arg(source);
        execute_command(&mut command).await
    }

    async fn list_machines(&self) -> Result<Vec<String>> {
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
            .await
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

async fn execute_command(command: &mut Command) -> Result<()> {
    command.stdout(Stdio::null());
    command.stderr(Stdio::piped());

    let mut child = command.spawn().with_context(|| "failed to execute lix")?;

    let stderr = child
        .stderr
        .take()
        .context("failed to capture command stderr")?;

    let mut renderer = LixRenderer::new();

    let mut lines = BufReader::new(stderr).lines();
    let mut read_error = None;
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => renderer.handle_stderr_line(&line),
            Ok(None) => break,
            Err(error) => {
                read_error = Some(error);
                break;
            }
        }
    }

    let status = child.wait().await.with_context(|| "failed to wait for lix")?;

    if let Some(error) = read_error {
        log::warn!("failed reading stderr from lix: {error}");
    }

    renderer.finish(status.success());

    if status.success() {
        Ok(())
    } else {
        bail!("lix exited with status {status}");
    }
}
