use anyhow::Result;

use crate::cli::{Command as CliCommand, CommonBuildArgs};

#[derive(Clone, Copy)]
pub enum ArtifactKind {
    Iso,
    Sdcard,
}

#[derive(Clone, Copy)]
pub struct ActivationPlan {
    pub command: &'static str,
    pub set_profile: bool,
    pub reboot: bool,
}

impl ActivationPlan {
    pub const SWITCH: Self = Self {
        command: "switch",
        set_profile: true,
        reboot: false,
    };

    pub const BOOT: Self = Self {
        command: "boot",
        set_profile: true,
        reboot: false,
    };

    pub const REBOOT: Self = Self {
        command: "boot",
        set_profile: true,
        reboot: true,
    };

    pub const TEST: Self = Self {
        command: "test",
        set_profile: false,
        reboot: false,
    };

    pub const DRY_ACTIVATE: Self = Self {
        command: "dry-activate",
        set_profile: false,
        reboot: false,
    };
}

pub enum PlanMode {
    Deploy(ActivationPlan),
    BuildOnly,
    Artifact,
}

#[derive(Clone)]
pub struct HostPlan {
    pub hostname: String,
    pub installable: String,
    pub out_link_name: String,
    pub keep_going: bool,
}

pub struct PipelinePlan {
    pub hosts: Vec<HostPlan>,
    pub mode: PlanMode,
}

pub fn plan(command: &CliCommand) -> Result<PipelinePlan> {
    match command {
        CliCommand::Switch(args) => deploy_plan(args, ActivationPlan::SWITCH),
        CliCommand::Boot(args) => deploy_plan(args, ActivationPlan::BOOT),
        CliCommand::Reboot(args) => deploy_plan(args, ActivationPlan::REBOOT),
        CliCommand::Test(args) => deploy_plan(args, ActivationPlan::TEST),
        CliCommand::DryActivate(args) => deploy_plan(args, ActivationPlan::DRY_ACTIVATE),
        CliCommand::Build(args) => Ok(build_only_plan(args)),
        CliCommand::Iso(args) => Ok(artifact_plan(args, ArtifactKind::Iso)),
        CliCommand::Sdcard(args) => Ok(artifact_plan(args, ArtifactKind::Sdcard)),
    }
}

fn deploy_plan(args: &CommonBuildArgs, activation: ActivationPlan) -> Result<PipelinePlan> {
    Ok(PipelinePlan {
        hosts: host_plans(&args.hostnames, |hostname| HostPlan {
            hostname: hostname.to_string(),
            installable: format!(".#nixosConfigurations.{hostname}.config.system.build.toplevel"),
            out_link_name: format!("nixos-config-{hostname}"),
            keep_going: true,
        }),
        mode: PlanMode::Deploy(activation),
    })
}

fn build_only_plan(args: &CommonBuildArgs) -> PipelinePlan {
    PipelinePlan {
        hosts: host_plans(&args.hostnames, |hostname| HostPlan {
            hostname: hostname.to_string(),
            installable: format!(".#nixosConfigurations.{hostname}.config.system.build.toplevel"),
            out_link_name: format!("nixos-config-{hostname}"),
            keep_going: true,
        }),
        mode: PlanMode::BuildOnly,
    }
}

fn artifact_plan(args: &CommonBuildArgs, kind: ArtifactKind) -> PipelinePlan {
    PipelinePlan {
        hosts: host_plans(&args.hostnames, |hostname| {
            let (attr, out_link_name) = match kind {
                ArtifactKind::Iso => (
                    format!(".#nixosConfigurations.{hostname}.config.system.build.iso"),
                    format!("nixos-iso-{hostname}"),
                ),
                ArtifactKind::Sdcard => (
                    format!(".#nixosConfigurations.{hostname}.config.system.build.sdImage"),
                    format!("nixos-sdcard-{hostname}"),
                ),
            };

            HostPlan {
                hostname: hostname.to_string(),
                installable: attr,
                out_link_name,
                keep_going: false,
            }
        }),
        mode: PlanMode::Artifact,
    }
}

fn host_plans<F>(hostnames: &[String], mut map: F) -> Vec<HostPlan>
where
    F: FnMut(&str) -> HostPlan,
{
    hostnames.iter().map(|hostname| map(hostname)).collect()
}
