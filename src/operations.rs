use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use cmd_lib::{run_cmd, run_fun};
use tempfile::Builder;

use crate::{
    backend::{self, Backend, LogFormat},
    cli::Command as CliCommand,
};
use log::{info, warn};

pub fn execute(command: CliCommand) -> Result<()> {
    let backend_handle = backend::detect();
    let backend = backend_handle.as_ref();

    match command {
        CliCommand::Switch(args) => {
            deploy_activation(backend, &args.hostname, ActivationPlan::SWITCH)
        }
        CliCommand::Boot(args) => deploy_activation(backend, &args.hostname, ActivationPlan::BOOT),
        CliCommand::Reboot(args) => {
            deploy_activation(backend, &args.hostname, ActivationPlan::REBOOT)
        }
        CliCommand::Test(args) => deploy_activation(backend, &args.hostname, ActivationPlan::TEST),
        CliCommand::DryActivate(args) => {
            deploy_activation(backend, &args.hostname, ActivationPlan::DRY_ACTIVATE)
        }
        CliCommand::Build(args) => build_only(backend, &args.hostname),
        CliCommand::Iso(args) => build_artifact(backend, &args.hostname, ArtifactKind::Iso),
        CliCommand::Sdcard(args) => build_artifact(backend, &args.hostname, ArtifactKind::Sdcard),
    }
}

enum ArtifactKind {
    Iso,
    Sdcard,
}

fn build_artifact(backend: &dyn Backend, hostname: &str, kind: ArtifactKind) -> Result<()> {
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
        .build_installable(&attr, &out_link, false, LogFormat::BarWithLogs)
        .with_context(|| format!("failed to build {attr}"))?;

    let realized = resolve_store_path(&out_link)?;
    println!("{}", realized.display());
    Ok(())
}

fn build_only(backend: &dyn Backend, hostname: &str) -> Result<()> {
    let (store_path, _temp_dir) = build_toplevel(backend, hostname)?;
    info!("Build completed");
    println!("{}", store_path.display());
    Ok(())
}

fn deploy_activation(backend: &dyn Backend, hostname: &str, plan: ActivationPlan) -> Result<()> {
    let (store_path, _temp_dir) = build_toplevel(backend, hostname)?;

    let local_hostname = run_fun!(hostname)
        .context("failed to determine local hostname")?
        .trim()
        .to_string();
    let is_target_host = local_hostname == hostname;

    info!("Deploying target system configuration");

    if is_target_host {
        deploy_local(&store_path, plan)?;
    } else {
        deploy_remote(backend, hostname, &store_path, plan)?;
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

fn build_toplevel(backend: &dyn Backend, hostname: &str) -> Result<(PathBuf, tempfile::TempDir)> {
    info!("Building target system configuration");

    let temp_dir = Builder::new()
        .prefix("wat-deploy.")
        .tempdir()
        .context("failed to create temporary directory")?;

    let out_link = temp_dir.path().join(format!("nixos-config-{hostname}"));
    let attr = { format!(".#nixosConfigurations.{hostname}.config.system.build.toplevel") };

    backend
        .build_installable(&attr, &out_link, true, LogFormat::BarWithLogs)
        .with_context(|| format!("failed to build {attr}"))?;

    let realized = resolve_store_path(&out_link)?;
    Ok((realized, temp_dir))
}

fn resolve_store_path(link: &Path) -> Result<PathBuf> {
    if !link.exists() {
        bail!("build output link {} does not exist", link.display());
    }

    std::fs::canonicalize(link)
        .with_context(|| format!("failed to resolve store path for {}", link.display()))
}

fn deploy_local(store_path: &Path, plan: ActivationPlan) -> Result<()> {
    let store_path_str = store_path
        .to_str()
        .context("store path contains invalid UTF-8")?;

    if plan.set_profile {
        run_cmd!(sudo nix-env --profile /nix/var/nix/profiles/system --set $store_path_str)
            .context("failed to set system profile")?;
    }

    let switch_to_config = store_path.join("bin/switch-to-configuration");
    let switch_to_config = switch_to_config
        .to_str()
        .context("switch-to-configuration path contains invalid UTF-8")?;
    let command = plan.command;
    run_cmd!(sudo $switch_to_config $command 1>&2)
        .with_context(|| format!("failed to run switch-to-configuration {}", plan.command))?;

    run_cmd!(sync).context("failed to sync filesystem")?;

    if plan.reboot {
        run_cmd!(sudo systemctl reboot).context("failed to reboot host")?;
    }

    Ok(())
}

fn deploy_remote(
    backend: &dyn Backend,
    hostname: &str,
    store_path: &Path,
    plan: ActivationPlan,
) -> Result<()> {
    let remote_store = format!("ssh://root@{hostname}");
    backend
        .copy_closure(
            store_path,
            remote_store.as_str(),
            true,
            LogFormat::BarWithLogs,
        )
        .with_context(|| format!("failed to copy store path to {hostname}"))?;

    let remote_host = format!("root@{hostname}");

    if plan.set_profile {
        let remote_cmd = format!(
            "nix-env --profile /nix/var/nix/profiles/system --set {}",
            store_path.display()
        );
        run_cmd!(ssh $remote_host $remote_cmd)
            .with_context(|| format!("failed to set remote profile on {hostname}"))?;
    }

    let remote_switch_cmd = format!(
        "{}/bin/switch-to-configuration {} && sync",
        store_path.display(),
        plan.command
    );
    run_cmd!(ssh $remote_host $remote_switch_cmd 1>&2)
        .with_context(|| format!("failed to switch configuration remotely on {hostname}"))?;

    if plan.reboot {
        run_cmd!(ssh $remote_host "systemctl reboot")
            .with_context(|| format!("failed to reboot remote host {hostname}"))?;
    }

    Ok(())
}
