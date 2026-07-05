use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tokio::{
    fs,
    process::Command,
};
use tempfile::Builder;

use crate::{
    backend::{self, Backend},
    cli::Command as CliCommand,
};
use log::{info, warn};

pub async fn execute(command: CliCommand) -> Result<()> {
    let backend_handle = backend::detect();
    let backend = backend_handle.as_ref();

    match command {
        CliCommand::Switch(args) => {
            deploy_activation(backend, &args.hostname, ActivationPlan::SWITCH).await
        }
        CliCommand::Boot(args) => deploy_activation(backend, &args.hostname, ActivationPlan::BOOT).await,
        CliCommand::Reboot(args) => {
            deploy_activation(backend, &args.hostname, ActivationPlan::REBOOT).await
        }
        CliCommand::Test(args) => deploy_activation(backend, &args.hostname, ActivationPlan::TEST).await,
        CliCommand::DryActivate(args) => {
            deploy_activation(backend, &args.hostname, ActivationPlan::DRY_ACTIVATE).await
        }
        CliCommand::Build(args) => build_only(backend, &args.hostname).await,
        CliCommand::Iso(args) => build_artifact(backend, &args.hostname, ArtifactKind::Iso).await,
        CliCommand::Sdcard(args) => build_artifact(backend, &args.hostname, ArtifactKind::Sdcard).await,
    }
}

enum ArtifactKind {
    Iso,
    Sdcard,
}

async fn build_artifact(backend: &dyn Backend, hostname: &str, kind: ArtifactKind) -> Result<()> {
    let temp_dir = Builder::new()
        .prefix("wat-deploy.")
        .tempdir()
        .context("failed to create temporary directory")?;

    let (attr, link_name, info_message) = match kind {
        ArtifactKind::Iso => (
            { format!(".#nixosConfigurations.{hostname}.config.system.build.iso") },
            format!("nixos-iso-{hostname}"),
            "Building iso image",
        ),
        ArtifactKind::Sdcard => (
            { format!(".#nixosConfigurations.{hostname}.config.system.build.sdImage") },
            format!("nixos-sdcard-{hostname}"),
            "Building sdcard image",
        ),
    };

    info!("{info_message}");

    let out_link = temp_dir.path().join(link_name);
    backend
        .build_installable(&attr, &out_link, false)
        .await
        .with_context(|| format!("failed to build {attr}"))?;

    let realized = resolve_store_path(&out_link).await?;
    println!("{}", realized.display());
    Ok(())
}

async fn build_only(backend: &dyn Backend, hostname: &str) -> Result<()> {
    let (store_path, _temp_dir) = build_toplevel(backend, hostname).await?;
    info!("Build completed");
    println!("{}", store_path.display());
    Ok(())
}

async fn deploy_activation(backend: &dyn Backend, hostname: &str, plan: ActivationPlan) -> Result<()> {
    let (store_path, _temp_dir) = build_toplevel(backend, hostname).await?;

    let local_hostname = run_command_output("hostname", &[]).await?.trim().to_string();
    let is_target_host = local_hostname == hostname;

    info!("Deploying target system configuration");

    if is_target_host {
        deploy_local(&store_path, plan).await?;
    } else {
        deploy_remote(backend, hostname, &store_path, plan).await?;
    }

    if plan.reboot {
        warn!("Reboot triggered");
    }

    info!("Update completed");
    Ok(())
}

#[derive(Clone, Copy)]
struct ActivationPlan {
    command: &'static str,
    set_profile: bool,
    reboot: bool,
}

impl ActivationPlan {
    const SWITCH: Self = Self {
        command: "switch",
        set_profile: true,
        reboot: false,
    };

    const BOOT: Self = Self {
        command: "boot",
        set_profile: true,
        reboot: false,
    };

    const REBOOT: Self = Self {
        command: "boot",
        set_profile: true,
        reboot: true,
    };

    const TEST: Self = Self {
        command: "test",
        set_profile: false,
        reboot: false,
    };

    const DRY_ACTIVATE: Self = Self {
        command: "dry-activate",
        set_profile: false,
        reboot: false,
    };
}

async fn build_toplevel(backend: &dyn Backend, hostname: &str) -> Result<(PathBuf, tempfile::TempDir)> {
    info!("Building target system configuration");

    let temp_dir = Builder::new()
        .prefix("wat-deploy.")
        .tempdir()
        .context("failed to create temporary directory")?;

    let out_link = temp_dir.path().join(format!("nixos-config-{hostname}"));
    let attr = { format!(".#nixosConfigurations.{hostname}.config.system.build.toplevel") };

    backend
        .build_installable(&attr, &out_link, true)
        .await
        .with_context(|| format!("failed to build {attr}"))?;

    let realized = resolve_store_path(&out_link).await?;
    Ok((realized, temp_dir))
}

async fn resolve_store_path(link: &Path) -> Result<PathBuf> {
    if !fs::try_exists(link).await.context("failed to check build output link")? {
        bail!("build output link {} does not exist", link.display());
    }

    fs::canonicalize(link)
        .await
        .with_context(|| format!("failed to resolve store path for {}", link.display()))
}

async fn deploy_local(store_path: &Path, plan: ActivationPlan) -> Result<()> {
    let store_path_str = store_path
        .to_str()
        .context("store path contains invalid UTF-8")?;

    if plan.set_profile {
        run_command_status(
            "sudo",
            &["nix-env", "--profile", "/nix/var/nix/profiles/system", "--set", store_path_str],
        )
        .await
        .context("failed to set system profile")?;
    }

    let switch_to_config = store_path.join("bin/switch-to-configuration");
    let switch_to_config = switch_to_config
        .to_str()
        .context("switch-to-configuration path contains invalid UTF-8")?;
    let command = plan.command;
    run_command_status("sudo", &[switch_to_config, command])
        .await
        .with_context(|| format!("failed to run switch-to-configuration {}", plan.command))?;

    run_command_status("sync", &[]).await.context("failed to sync filesystem")?;

    if plan.reboot {
        run_command_status("sudo", &["systemctl", "reboot"]).await.context("failed to reboot host")?;
    }

    Ok(())
}

async fn deploy_remote(
    backend: &dyn Backend,
    hostname: &str,
    store_path: &Path,
    plan: ActivationPlan,
) -> Result<()> {
    let remote_store = format!("ssh://root@{hostname}");
    backend
        .copy_closure(store_path, remote_store.as_str(), true)
        .await
        .with_context(|| format!("failed to copy store path to {hostname}"))?;

    let remote_host = format!("root@{hostname}");

    if plan.set_profile {
        let remote_cmd = format!(
            "nix-env --profile /nix/var/nix/profiles/system --set {}",
            store_path.display()
        );
        run_command_status("ssh", &[remote_host.as_str(), remote_cmd.as_str()])
            .await
            .with_context(|| format!("failed to set remote profile on {hostname}"))?;
    }

    let remote_switch_cmd = format!(
        "{}/bin/switch-to-configuration {} && sync",
        store_path.display(),
        plan.command
    );
    run_command_status("ssh", &[remote_host.as_str(), remote_switch_cmd.as_str()])
        .await
        .with_context(|| format!("failed to switch configuration remotely on {hostname}"))?;

    if plan.reboot {
        run_command_status("ssh", &[remote_host.as_str(), "systemctl reboot"])
            .await
            .with_context(|| format!("failed to reboot remote host {hostname}"))?;
    }

    Ok(())
}

async fn run_command_output(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .with_context(|| format!("failed to run {program}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{program} exited with status {}: {}", output.status, stderr.trim());
    }

    String::from_utf8(output.stdout).context("command output contained invalid UTF-8")
}

async fn run_command_status(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .stderr(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .status()
        .await
        .with_context(|| format!("failed to run {program}"))?;

    if status.success() {
        Ok(())
    } else {
        bail!("{program} exited with status {status}")
    }
}
