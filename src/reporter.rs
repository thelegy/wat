use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::Level;

use crate::{
    event::{ArtifactEvent, Event, EventStream, HostEvent, PipelineEvent},
    reducer::{ActivitySnapshot, HostSnapshot, PipelineReducer, PipelineSnapshot},
};

#[async_trait]
pub trait Reporter: Send {
    async fn run(&mut self, events: EventStream) -> Result<()>;
}

pub struct IndicatifReporter {
    reducer: PipelineReducer,
    progress: MultiProgress,
    summary_bar: ProgressBar,
    host_bars: HashMap<String, ProgressBar>,
    activity_bars: HashMap<u64, ProgressBar>,
    last_summary: String,
}

impl IndicatifReporter {
    pub fn new() -> Self {
        let progress = MultiProgress::new();
        let summary_bar = progress.add(ProgressBar::new_spinner());
        summary_bar.set_style(summary_style());
        summary_bar.enable_steady_tick(std::time::Duration::from_millis(120));

        Self {
            reducer: PipelineReducer::new(),
            progress,
            summary_bar,
            host_bars: HashMap::new(),
            activity_bars: HashMap::new(),
            last_summary: String::new(),
        }
    }

    fn render(&mut self, snapshot: PipelineSnapshot) {
        self.render_summary(&snapshot.summary);
        self.render_hosts(&snapshot.hosts);
        self.render_activities(&snapshot.activities);

        if snapshot.finished {
            self.summary_bar.finish_and_clear();
        }
    }

    fn render_summary(&mut self, summary: &str) {
        if summary != self.last_summary {
            self.summary_bar.set_message(summary.to_string());
            self.last_summary = summary.to_string();
        }
        self.summary_bar.tick();
    }

    fn render_hosts(&mut self, hosts: &[HostSnapshot]) {
        let mut seen = HashMap::new();

        for host in hosts {
            seen.insert(host.host.clone(), ());
            let bar = self.host_bars.entry(host.host.clone()).or_insert_with(|| {
                let bar = self.progress.add(ProgressBar::new_spinner());
                bar.set_style(host_style());
                bar.enable_steady_tick(std::time::Duration::from_millis(120));
                bar
            });

            let mut message = host.message.clone();
            if let Some(phase) = host.phase
                && message.is_empty()
            {
                message = format_phase(phase, &host.host);
            }
            bar.set_message(message);

            if host.finished {
                if host.success.unwrap_or(false) {
                    bar.finish_and_clear();
                } else {
                    bar.abandon_with_message(format!("{} failed", host.host));
                }
            }
        }

        let stale_hosts: Vec<String> = self
            .host_bars
            .keys()
            .filter(|host| !seen.contains_key(*host))
            .cloned()
            .collect();

        for host in stale_hosts {
            if let Some(bar) = self.host_bars.remove(&host) {
                self.progress.remove(&bar);
            }
        }
    }

    fn render_activities(&mut self, activities: &[ActivitySnapshot]) {
        let mut seen = HashMap::new();

        for activity in activities.iter().filter(|activity| activity.visible) {
            seen.insert(activity.id, ());
            let bar = self.activity_bars.entry(activity.id).or_insert_with(|| {
                let bar = self.progress.add(ProgressBar::new_spinner());
                bar.set_style(activity_style());
                bar.enable_steady_tick(std::time::Duration::from_millis(120));
                bar
            });

            if activity.expected > 0 {
                let len = activity.expected.max(activity.done).max(1);
                if bar.length().unwrap_or(0) != len {
                    bar.set_length(len);
                }
                bar.set_position(activity.done.min(len));
            }

            bar.set_message(activity_message(activity));
        }

        let stale_activities: Vec<u64> = self
            .activity_bars
            .keys()
            .filter(|id| !seen.contains_key(*id))
            .copied()
            .collect();

        for id in stale_activities {
            if let Some(bar) = self.activity_bars.remove(&id) {
                self.progress.remove(&bar);
            }
        }
    }

    fn log(&self, level: Level, message: &str) {
        self.summary_bar.suspend(|| match level {
            Level::Error => log::error!("{message}"),
            Level::Warn => log::warn!("{message}"),
            Level::Info => log::info!("{message}"),
            Level::Debug => log::debug!("{message}"),
            Level::Trace => log::trace!("{message}"),
        });
    }

    fn emit_artifact(&self, artifact: &ArtifactEvent) {
        self.summary_bar.suspend(|| match artifact {
            ArtifactEvent::Ready { host, path } => {
                let _ = host;
                println!("{}", path.display());
            }
        });
    }
}

#[async_trait]
impl Reporter for IndicatifReporter {
    async fn run(&mut self, mut events: EventStream) -> Result<()> {
        while let Some(envelope) = events.recv().await {
            self.reducer.apply(&envelope);

            match &envelope.event {
                Event::Log(log) => self.log(log.level, &log.message),
                Event::Artifact(artifact) => self.emit_artifact(artifact),
                Event::Pipeline(PipelineEvent::Finished { .. }) => {}
                Event::Host(HostEvent::Finished { .. }) => {}
                _ => {}
            }

            let snapshot = self.reducer.snapshot();
            self.render(snapshot);
        }

        Ok(())
    }
}

fn summary_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.cyan} {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
}

fn host_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.green} {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
}

fn activity_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.yellow} {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
}

fn format_phase(phase: crate::event::PipelinePhase, host: &str) -> String {
    match phase {
        crate::event::PipelinePhase::Evaluate => format!("evaluating {host}"),
        crate::event::PipelinePhase::Build => format!("building {host}"),
        crate::event::PipelinePhase::Deploy => format!("deploying {host}"),
    }
}

fn activity_message(activity: &ActivitySnapshot) -> String {
    let mut message = activity.text.clone();

    if let Some(host) = &activity.host {
        if !message.is_empty() {
            message = format!("{host}: {message}");
        } else {
            message = host.clone();
        }
    }

    if !activity.last_line.is_empty() {
        if !message.is_empty() {
            message.push_str(": ");
        }
        message.push_str(&activity.last_line);
    }

    if activity.expected > 0 {
        let counts = format!("{} / {}", activity.done, activity.expected);
        if message.is_empty() {
            message = counts;
        } else {
            message.push_str(" [");
            message.push_str(&counts);
            message.push(']');
        }
    }

    if activity.running > 0 || activity.failed > 0 {
        let mut counters = Vec::new();
        if activity.running > 0 {
            counters.push(format!("{} running", activity.running));
        }
        if activity.failed > 0 {
            counters.push(format!("{} failed", activity.failed));
        }
        if !counters.is_empty() {
            if !message.is_empty() {
                message.push_str(" (");
                message.push_str(&counters.join(", "));
                message.push(')');
            } else {
                message = counters.join(", ");
            }
        }
    }

    message
}
