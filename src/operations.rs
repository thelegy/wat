use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use tempfile::{Builder, TempDir};
use tokio::{
    process::Command,
    task::JoinSet,
};

use crate::{
    backend::{self, Backend},
    cli::Command as CliCommand,
    event::{
        ArtifactEvent, Event, EventBus, EventSender, HostEvent, LogEvent, PipelineEvent,
        PipelinePhase,
    },
    planner::{self, HostPlan, PipelinePlan, PlanMode},
    reporter::{IndicatifReporter, Reporter},
};

pub async fn execute(mut command: CliCommand, report: crate::results::Report) -> Result<()> {
    let config = crate::config::Config::load(Path::new("."))?;
    let backend = backend::detect();
    if let Some(args) = command.build_args_mut() {
        if args.all {
            args.hostnames = backend.list_machines().await.context("failed to list hosts for --all")?;
            if args.hostnames.is_empty() {
                bail!("no hosts found in nixosConfigurations");
            }
        }
    }
    let plan = planner::plan(&command)?;
    report.select(plan.hosts.iter().map(|host| host.hostname.clone()));
    let (bus, events) = EventBus::new(512);

    let mut reporter = IndicatifReporter::new();
    let reporter_handle = tokio::spawn(async move { reporter.run(events).await });

    let result = run_pipeline(backend, plan, config, bus.sender(), report).await;
    drop(bus);

    let reporter_result = reporter_handle
        .await
        .context("reporter task failed to join")?;
    reporter_result?;

    result
}

async fn run_pipeline(
    backend: Arc<dyn Backend + Send + Sync>,
    plan: PipelinePlan,
    config: crate::config::Config,
    events: EventSender,
    report: crate::results::Report,
) -> Result<()> {
    let hostnames: Vec<String> = plan
        .hosts
        .iter()
        .map(|host| host.hostname.clone())
        .collect();
    let stages = hostnames
        .iter()
        .map(|host| {
            let mut phases = vec![PipelinePhase::Evaluate, PipelinePhase::Build];
            if matches!(plan.mode, PlanMode::Deploy(_)) {
                if config.connection_for(host) != crate::config::Connection::Sudo {
                    phases.push(PipelinePhase::Upload);
                }
                phases.push(PipelinePhase::Deploy);
            }
            (host.clone(), phases)
        })
        .collect();
    events
        .send(Event::Pipeline(PipelineEvent::Planned { stages }))
        .await?;
    events
        .send(Event::Pipeline(PipelineEvent::Started { hosts: hostnames }))
        .await?;

    let (builds, mut errors) = build_hosts(backend.clone(), &plan.hosts, events.clone(), report).await;

    if !(plan.fail_early && !errors.is_empty()) && !builds.is_empty() {
        if let PlanMode::Deploy(activation) = plan.mode {
            events
                .send(Event::Pipeline(PipelineEvent::PhaseStarted(PipelinePhase::Deploy)))
                .await?;
            let deploy_errors = deploy_hosts(
                backend, &builds, activation, &config, events.clone(), plan.fail_early,
            ).await;
            events
                .send(Event::Pipeline(PipelineEvent::PhaseFinished {
                    phase: PipelinePhase::Deploy,
                    success: deploy_errors.is_empty(),
                }))
                .await?;
            errors.extend(deploy_errors);
        }
    }

    events
        .send(Event::Pipeline(PipelineEvent::Finished { success: errors.is_empty() }))
        .await?;
    if errors.is_empty() {
        Ok(())
    } else {
        Err(PipelineFailed(errors.iter().map(|error| format!("{error:#}")).collect::<Vec<_>>().join("\n")).into())
    }
}

async fn build_hosts(
    backend: Arc<dyn Backend + Send + Sync>,
    hosts: &[HostPlan],
    events: EventSender,
    report: crate::results::Report,
) -> (Vec<BuildOutcome>, Vec<anyhow::Error>) {
    let mut tasks = JoinSet::new();

    for host in hosts.iter().cloned() {
        let backend = backend.clone();
        let events = events.clone();
        let report = report.clone();
        tasks.spawn(async move {
            let hostname = host.hostname.clone();
            let result = build_host(backend, host, events).await;
            report.host_finished(hostname, &result);
            result
        });
    }

    collect_join_set(tasks).await
}

async fn build_host(
    backend: Arc<dyn Backend + Send + Sync>,
    host: HostPlan,
    events: EventSender,
) -> Result<BuildOutcome> {
    let host_name = host.hostname.clone();
    let events = events
        .with_host(host_name.clone())
        .with_phase(PipelinePhase::Evaluate);
    let mut phase = PipelinePhase::Evaluate;
    events
        .send(Event::Host(HostEvent::Started {
            host: host_name.clone(),
            phase,
        }))
        .await?;

    let result = async {
        let temp_dir = Builder::new()
            .prefix("wat-deploy.")
            .tempdir()
            .context("failed to create temporary directory")?;
        let out_link = temp_dir.path().join(&host.out_link_name);

        let derivation = backend
            .evaluate_installable(&host.installable, events.clone())
            .await
            .with_context(|| format!("failed to evaluate {}", host.installable))?;
        events.send(Event::Host(HostEvent::Finished {
            host: host_name.clone(), phase, success: true,
        })).await?;
        phase = PipelinePhase::Build;
        let events = events.with_phase(phase);
        events.send(Event::Host(HostEvent::Started {
            host: host_name.clone(), phase,
        })).await?;

        backend
            .build_installable(
                &derivation,
                &out_link,
                host.keep_going,
                events.clone(),
            )
            .await
            .with_context(|| format!("failed to build {}", host.installable))?;

        let store_path = resolve_store_path(&out_link).await?;

        events
            .send(Event::Artifact(ArtifactEvent::Ready {
                host: host_name.clone(),
                path: store_path.clone(),
            }))
            .await?;

        events
            .send(Event::Host(HostEvent::Finished {
                host: host_name.clone(),
                phase: PipelinePhase::Build,
                success: true,
            }))
            .await?;

        Ok(BuildOutcome {
            host: host_name.clone(),
            store_path,
            _temp_dir: temp_dir,
        })
    }
    .await;

    if result.is_err() {
        let _ = events.with_phase(phase)
            .send(Event::Host(HostEvent::Finished {
                host: host_name,
                phase,
                success: false,
            }))
            .await;
    }

    result
}

async fn deploy_hosts(
    backend: Arc<dyn Backend + Send + Sync>,
    builds: &[BuildOutcome],
    activation: planner::ActivationPlan,
    config: &crate::config::Config,
    events: EventSender,
    fail_early: bool,
) -> Vec<anyhow::Error> {
    let mut pending = builds.iter();
    let mut tasks = JoinSet::new();
    let mut errors = Vec::new();
    loop {
        while tasks.len() < deploy_concurrency() && !(fail_early && !errors.is_empty()) {
            let Some(build) = pending.next() else { break };
            let backend = backend.clone();
            let events = events.clone();
            let hostname = build.host.clone();
            let store_path = build.store_path.clone();
            let connection = config.connection_for(&hostname);
            tasks.spawn(async move {
                deploy_host(backend, hostname, store_path, activation, connection, events).await
            });
        }
        let Some(result) = tasks.join_next().await else { break };
        match result {
            Ok(Ok(())) => {},
            Ok(Err(error)) => errors.push(error),
            Err(error) => errors.push(anyhow::anyhow!("task join failed: {error}")),
        }
    }
    errors
}

async fn deploy_host(
    backend: Arc<dyn Backend + Send + Sync>,
    hostname: String,
    store_path: PathBuf,
    activation: planner::ActivationPlan,
    connection: crate::config::Connection,
    events: EventSender,
) -> Result<()> {
    let host_name = hostname.clone();
    let events = events
        .with_host(host_name.clone())
        .with_phase(PipelinePhase::Deploy);
    events
        .send(Event::Host(HostEvent::Started {
            host: host_name.clone(),
            phase: PipelinePhase::Deploy,
        }))
        .await?;

    let result = async {
        if connection == crate::config::Connection::Sudo {
            deploy_local(&store_path, activation, &events).await?;
        } else {
            deploy_remote(
                backend.clone(),
                &host_name,
                &store_path,
                activation,
                events.clone(),
            )
            .await?;
        }

        events
            .send(Event::Host(HostEvent::Finished {
                host: host_name.clone(),
                phase: PipelinePhase::Deploy,
                success: true,
            }))
            .await?;
        Ok(())
    }
    .await;

    if result.is_err() {
        let _ = events
            .send(Event::Host(HostEvent::Finished {
                host: host_name,
                phase: PipelinePhase::Deploy,
                success: false,
            }))
            .await;
    }

    result
}

struct BuildOutcome {
    host: String,
    store_path: PathBuf,
    _temp_dir: TempDir,
}

async fn collect_join_set<T: 'static>(mut tasks: JoinSet<Result<T>>) -> (Vec<T>, Vec<anyhow::Error>) {
    let mut results = Vec::new();
    let mut errors = Vec::new();
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(item)) => results.push(item),
            Ok(Err(error)) => errors.push(error),
            Err(error) => errors.push(anyhow::anyhow!("task join failed: {error}")),
        }
    }
    (results, errors)
}

fn deploy_concurrency() -> usize {
    std::env::var("WAT_DEPLOY_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(4)
}

async fn resolve_store_path(link: &Path) -> Result<PathBuf> {
    if !tokio::fs::try_exists(link)
        .await
        .context("failed to check build output link")?
    {
        bail!("build output link {} does not exist", link.display());
    }

    tokio::fs::canonicalize(link)
        .await
        .with_context(|| format!("failed to resolve store path for {}", link.display()))
}

async fn deploy_local(
    store_path: &Path,
    plan: planner::ActivationPlan,
    events: &EventSender,
) -> Result<()> {
    let store_path_str = store_path
        .to_str()
        .context("store path contains invalid UTF-8")?;

    if plan.set_profile {
        run_command_status(
            "sudo",
            &[
                "nix-env",
                "--profile",
                "/nix/var/nix/profiles/system",
                "--set",
                store_path_str,
            ],
            events,
        )
        .await
        .context("failed to set system profile")?;
    }

    let switch_to_config = store_path.join("bin/switch-to-configuration");
    let switch_to_config = switch_to_config
        .to_str()
        .context("switch-to-configuration path contains invalid UTF-8")?;
    deployment_stage(
        events,
        PipelinePhase::Activate,
        run_activation("sudo", &[switch_to_config, plan.command], events),
    )
    .await
    .with_context(|| format!("failed to run switch-to-configuration {}", plan.command))?;

    run_command_status("sync", &[], events)
        .await
        .context("failed to sync filesystem")?;

    if plan.reboot {
        deployment_stage(
            events,
            PipelinePhase::Reboot,
            run_command_status("sudo", &["systemctl", "reboot"], events),
        )
        .await
        .context("failed to reboot host")?;
    }

    Ok(())
}

async fn deploy_remote(
    backend: Arc<dyn Backend + Send + Sync>,
    hostname: &str,
    store_path: &Path,
    plan: planner::ActivationPlan,
    events: EventSender,
) -> Result<()> {
    let remote_store = format!("ssh://root@{hostname}");
    deployment_stage(
        &events,
        PipelinePhase::Upload,
        backend.copy_closure(store_path, remote_store.as_str(), true, events.clone()),
    )
    .await
    .with_context(|| format!("failed to copy store path to {hostname}"))?;

    let remote_host = format!("root@{hostname}");

    if plan.set_profile {
        let remote_cmd = format!(
            "nix-env --profile /nix/var/nix/profiles/system --set {}",
            store_path.display()
        );
        run_command_status("ssh", &[remote_host.as_str(), remote_cmd.as_str()], &events)
            .await
            .with_context(|| format!("failed to set remote profile on {hostname}"))?;
    }

    let remote_switch_cmd = format!(
        "{}/bin/switch-to-configuration {}",
        store_path.display(),
        plan.command
    );
    deployment_stage(
        &events,
        PipelinePhase::Activate,
        run_activation(
            "ssh",
            &[remote_host.as_str(), remote_switch_cmd.as_str()],
            &events,
        ),
    )
    .await
    .with_context(|| format!("failed to switch configuration remotely on {hostname}"))?;

    run_command_status("ssh", &[remote_host.as_str(), "sync"], &events)
        .await
        .with_context(|| format!("failed to sync remote filesystem on {hostname}"))?;

    if plan.reboot {
        deployment_stage(
            &events,
            PipelinePhase::Reboot,
            run_command_status("ssh", &[remote_host.as_str(), "systemctl reboot"], &events),
        )
        .await
        .with_context(|| format!("failed to reboot remote host {hostname}"))?;
    }

    Ok(())
}

// The reporter already presents pipeline failures grouped by host and stage.
#[derive(Debug)]
pub(crate) struct PipelineFailed(pub(crate) String);

impl std::fmt::Display for PipelineFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PipelineFailed {}

pub fn failure_reported(error: &anyhow::Error) -> bool {
    error.is::<PipelineFailed>()
}

async fn run_activation(program: &str, args: &[&str], events: &EventSender) -> Result<()> {
    let mut command = Command::new(program);
    command.args(args);
    let status = crate::recording::execute_visible(&mut command, events).await?;
    activation_result(status, events).await
}

async fn activation_result(status: std::process::ExitStatus, events: &EventSender) -> Result<()> {
    if status.code() == Some(4) {
        events.with_phase(PipelinePhase::Activate).send(Event::Log(LogEvent {
            level: log::Level::Warn,
            message: "Configuration activated, but activation reported service errors (exit status 4)".into(),
        })).await?;
        Ok(())
    } else if status.success() {
        Ok(())
    } else {
        bail!("activation exited with status {status}")
    }
}

async fn run_command_status(program: &str, args: &[&str], events: &EventSender) -> Result<()> {
    let mut command = Command::new(program);
    command.args(args);
    let status = crate::recording::execute_visible(&mut command, events).await?;

    if status.success() {
        Ok(())
    } else {
        bail!("{program} exited with status {status}")
    }
}

async fn deployment_stage(
    events: &EventSender,
    phase: PipelinePhase,
    operation: impl std::future::Future<Output = Result<()>>,
) -> Result<()> {
    let host = events
        .scope()
        .host
        .clone()
        .context("deployment requires a host")?;
    events
        .send(Event::Host(HostEvent::Started {
            host: host.clone(),
            phase,
        }))
        .await?;
    let result = operation.await;
    events
        .send(Event::Host(HostEvent::Finished {
            host,
            phase,
            success: result.is_ok(),
        }))
        .await?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    #[tokio::test]
    async fn service_errors_complete_activation_with_a_host_warning() {
        let (bus, mut stream) = EventBus::new(16);
        let events = bus.sender().with_host("sirrah");
        deployment_stage(&events, PipelinePhase::Activate,
            activation_result(std::process::ExitStatus::from_raw(4 << 8), &events))
            .await.unwrap();
        assert!(matches!(stream.recv().await.unwrap().event,
            Event::Host(HostEvent::Started { .. })));
        let warning = stream.recv().await.unwrap();
        assert_eq!(warning.scope.host.as_deref(), Some("sirrah"));
        assert_eq!(warning.scope.phase, Some(PipelinePhase::Activate));
        assert!(matches!(warning.event, Event::Log(LogEvent { level: log::Level::Warn, .. })));
        assert!(matches!(stream.recv().await.unwrap().event,
            Event::Host(HostEvent::Finished { success: true, .. })));
        for code in [1, 2, 3, 255] {
            assert!(activation_result(std::process::ExitStatus::from_raw(code << 8), &events).await.is_err());
        }
    }
}
