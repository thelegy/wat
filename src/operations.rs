use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use cmd_lib::{run_cmd, run_fun};
use tempfile::Builder;

use crate::{
    backend::Backend,
    cli::Command as CliCommand,
    logging::{print_info, print_warning},
};

pub fn execute(command: CliCommand) -> Result<()> {
    let backend = Backend::detect();

    match command {
        CliCommand::Switch(args) => {
            deploy_activation(&backend, &args.hostname, ActivationPlan::SWITCH)
        }
        CliCommand::Boot(args) => deploy_activation(&backend, &args.hostname, ActivationPlan::BOOT),
        CliCommand::Reboot(args) => {
            deploy_activation(&backend, &args.hostname, ActivationPlan::REBOOT)
        }
        CliCommand::Test(args) => deploy_activation(&backend, &args.hostname, ActivationPlan::TEST),
        CliCommand::DryActivate(args) => {
            deploy_activation(&backend, &args.hostname, ActivationPlan::DRY_ACTIVATE)
        }
        CliCommand::Build(args) => build_only(&backend, &args.hostname),
        CliCommand::Iso(args) => build_artifact(&backend, &args.hostname, ArtifactKind::Iso),
        CliCommand::Sdcard(args) => build_artifact(&backend, &args.hostname, ArtifactKind::Sdcard),
    }
}

enum ArtifactKind {
    Iso,
    Sdcard,
}

fn build_artifact(backend: &Backend, hostname: &str, kind: ArtifactKind) -> Result<()> {
    let temp_dir = Builder::new()
        .prefix("wat-deploy.")
        .tempdir()
        .context("failed to create temporary directory")?;

    let (attr, link_name, info_message) = match kind {
        ArtifactKind::Iso => (
            backend.iso_attr(hostname),
            format!("nixos-iso-{hostname}"),
            "Building iso image",
        ),
        ArtifactKind::Sdcard => (
            backend.sdcard_attr(hostname),
            format!("nixos-sdcard-{hostname}"),
            "Building sdcard image",
        ),
    };

    print_info(info_message);

    let out_link = temp_dir.path().join(link_name);
    let binary = backend.binary();

    run_nix(binary, |cmd| {
        cmd.args(["--log-format", "bar-with-logs", "build"])
            .arg(&attr)
            .arg("--out-link")
            .arg(&out_link);
    })
    .with_context(|| format!("failed to build {attr}"))?;

    let realized = resolve_store_path(&out_link)?;
    println!("{}", realized.display());
    Ok(())
}

fn build_only(backend: &Backend, hostname: &str) -> Result<()> {
    let (store_path, _temp_dir) = build_toplevel(backend, hostname)?;
    print_info("Build completed");
    println!("{}", store_path.display());
    Ok(())
}

fn deploy_activation(backend: &Backend, hostname: &str, plan: ActivationPlan) -> Result<()> {
    let (store_path, _temp_dir) = build_toplevel(backend, hostname)?;

    let local_hostname = run_fun!(hostname)
        .context("failed to determine local hostname")?
        .trim()
        .to_string();
    let is_target_host = local_hostname == hostname;

    print_info("Deploying target system configuration");

    if is_target_host {
        deploy_local(&store_path, plan)?;
    } else {
        deploy_remote(backend, hostname, &store_path, plan)?;
    }

    if plan.reboot {
        print_warning("Reboot triggered");
    }

    print_info("Update completed");
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

fn build_toplevel(backend: &Backend, hostname: &str) -> Result<(PathBuf, tempfile::TempDir)> {
    print_info("Building target system configuration");

    let temp_dir = Builder::new()
        .prefix("wat-deploy.")
        .tempdir()
        .context("failed to create temporary directory")?;

    let out_link = temp_dir.path().join(format!("nixos-config-{hostname}"));
    let attr = backend.toplevel_attr(hostname);

    let binary = backend.binary();

    run_nix(binary, |cmd| {
        cmd.args(["--log-format", "bar-with-logs", "--keep-going", "build"])
            .arg(&attr)
            .arg("--out-link")
            .arg(&out_link);
    })
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
    run_cmd!(sudo $switch_to_config $command)
        .with_context(|| format!("failed to run switch-to-configuration {}", plan.command))?;

    run_cmd!(sync).context("failed to sync filesystem")?;

    if plan.reboot {
        run_cmd!(sudo systemctl reboot).context("failed to reboot host")?;
    }

    Ok(())
}

fn deploy_remote(
    backend: &Backend,
    hostname: &str,
    store_path: &Path,
    plan: ActivationPlan,
) -> Result<()> {
    let attr = backend.toplevel_attr(hostname);
    let binary = backend.binary();
    let remote_store = format!("ssh://root@{hostname}");

    run_nix(binary, |cmd| {
        cmd.args([
            "--log-format",
            "bar-with-logs",
            "copy",
            "--substitute-on-destination",
            "--to",
        ])
        .arg(&remote_store)
        .arg(&attr);
    })
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
    run_cmd!(ssh $remote_host $remote_switch_cmd)
        .with_context(|| format!("failed to switch configuration remotely on {hostname}"))?;

    if plan.reboot {
        run_cmd!(ssh $remote_host "systemctl reboot")
            .with_context(|| format!("failed to reboot remote host {hostname}"))?;
    }

    Ok(())
}

fn run_nix(binary: &str, configure: impl FnOnce(&mut Command)) -> Result<()> {
    let mut command = Command::new(binary);
    configure(&mut command);
    let status = command
        .status()
        .with_context(|| format!("failed to execute {binary}"))?;
    if status.success() {
        Ok(())
    } else {
        bail!("{binary} exited with status {status}");
    }
}
