use std::{path::Path, process::Stdio};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

use crate::event::EventSender;

use super::{Backend, lix_parser::LixEventAdapter};

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
    async fn evaluate_installable(&self, installable: &str, events: EventSender) -> Result<String> {
        let mut command = self.command();
        command.args(["--log-format", "internal-json", "eval", "--raw"]);
        command.arg(format!("{installable}.drvPath"));
        let output = execute_command(&mut command, events, true).await?;
        let path = String::from_utf8(output).context("derivation path contained invalid UTF-8")?;
        let path = path.trim();
        if !path.starts_with("/nix/store/") || !path.ends_with(".drv") {
            bail!("invalid derivation path: {path:?}");
        }
        Ok(path.to_string())
    }

    async fn build_installable(
        &self,
        installable: &str,
        out_link: &Path,
        keep_going: bool,
        events: EventSender,
    ) -> Result<()> {
        let mut command = self.command();
        command.args(["--log-format", "internal-json", "build"]);
        if keep_going {
            command.arg("--keep-going");
        }
        command.arg(format!("{installable}^*"));
        command.arg("--out-link");
        command.arg(out_link);
        execute_command(&mut command, events, false).await.map(|_| ())
    }

    async fn copy_closure(
        &self,
        source: &Path,
        destination: &str,
        substitute_on_destination: bool,
        events: EventSender,
    ) -> Result<()> {
        let mut command = self.command();
        command.args(["--log-format", "internal-json", "copy"]);
        if substitute_on_destination {
            command.arg("--substitute-on-destination");
        }
        command.arg("--to");
        command.arg(destination);
        command.arg(source);
        execute_command(&mut command, events, false).await.map(|_| ())
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
                format!("failed to invoke {} to list hosts", self.binary())
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

async fn execute_command(command: &mut Command, events: EventSender, capture_stdout: bool) -> Result<Vec<u8>> {
    let id = crate::recording::start(command);
    let scope = events.scope().clone();
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn().with_context(|| "failed to execute lix")?;

    let stdout = child.stdout.take().context("missing stdout")?;
    let stdout_scope = scope.clone();
    let stdout_task = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut stdout = stdout;
        let mut bytes = [0; 8192];
        let mut output = Vec::new();
        loop {
            let count = stdout.read(&mut bytes).await?;
            if count == 0 {
                break;
            }
            crate::recording::raw(&stdout_scope, id, "stdout", &bytes[..count], true);
            if capture_stdout {
                output.extend_from_slice(&bytes[..count]);
            }
        }
        Ok::<_, std::io::Error>(output)
    });
    let stderr = child
        .stderr
        .take()
        .context("failed to capture command stderr")?;

    let mut renderer = LixEventAdapter::new(events.unrecorded());

    let mut lines = BufReader::new(stderr);
    let mut bytes = Vec::new();
    let mut read_error = None;
    loop {
        bytes.clear();
        match lines.read_until(b'\n', &mut bytes).await {
            Ok(0) => break,
            Ok(_) => {
                crate::recording::raw(&scope, id, "stderr", &bytes, true);
                renderer
                    .handle_stderr_line(String::from_utf8_lossy(&bytes).trim_end_matches('\n'))
                    .await?;
            }
            Err(error) => {
                read_error = Some(error);
                break;
            }
        }
    }

    let status = child
        .wait()
        .await
        .with_context(|| "failed to wait for lix")?;

    let output = stdout_task.await??;
    crate::recording::write(
        serde_json::json!({"kind": "exit", "command": id, "success": status.success(), "code": status.code()}),
    );
    if let Some(error) = read_error {
        renderer
            .log_warning(&format!("failed reading stderr from lix: {error}"))
            .await?;
    }

    renderer.finish(status.success());

    if status.success() {
        Ok(output)
    } else {
        bail!("lix exited with status {status}");
    }
}
