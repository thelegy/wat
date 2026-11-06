use std::{collections::HashMap, io::Write};

use anyhow::Result;
use async_trait::async_trait;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::Level;

use crate::{
    event::{ArtifactEvent, Event, EventScope, EventStream, HostEvent, PipelineEvent},
    palette,
    reducer::{ActivitySnapshot, MachineColumn, PipelineReducer, PipelineSnapshot},
};

#[async_trait]
pub trait Reporter: Send {
    async fn run(&mut self, events: EventStream) -> Result<()>;
}

pub struct IndicatifReporter {
    reducer: PipelineReducer,
    progress: MultiProgress,
    summary_bar: ProgressBar,
    machine_bar: ProgressBar,
    activity_bars: HashMap<u64, ProgressBar>,
    last_summary: String,
    host_width: usize,
    last_machines: Vec<MachineColumn>,
    last_machine_line: String,
}

impl IndicatifReporter {
    pub fn new() -> Self {
        let progress = MultiProgress::new();
        let summary_bar = progress.add(ProgressBar::new_spinner());
        summary_bar.set_style(summary_style());
        let machine_bar = progress.add(ProgressBar::new_spinner());
        machine_bar.set_style(summary_style());
        machine_bar.enable_steady_tick(std::time::Duration::from_millis(120));

        Self {
            reducer: PipelineReducer::new(),
            progress,
            summary_bar,
            machine_bar,
            activity_bars: HashMap::new(),
            last_summary: String::new(),
            host_width: 0,
            last_machines: Vec::new(),
            last_machine_line: String::new(),
        }
    }

    fn render(&mut self, snapshot: PipelineSnapshot) {
        self.render_summary(&snapshot.summary);
        self.render_machines(&snapshot.machines);
        self.render_activities(&snapshot.activities);
    }

    fn render_machines(&mut self, columns: &[MachineColumn]) {
        if self.last_machines == columns {
            return;
        }
        let display = machine_display(columns);
        self.machine_bar.set_style(display.style);
        self.machine_bar.set_message(display.labels);
        self.last_machine_line = display.plain;
        self.last_machines = columns.to_vec();
    }

    fn finish(&mut self, output: &mut impl Write) -> Result<()> {
        for (_, bar) in self.activity_bars.drain() {
            bar.finish_and_clear();
        }
        self.summary_bar.finish_and_clear();
        for summary in self.reducer.failure_summaries() {
            if self.progress.is_hidden() {
                writeln!(output, "{summary}")?;
            } else {
                self.progress.println(summary)?;
            }
        }
        let snapshot = self.reducer.snapshot();
        if snapshot.machines.iter().any(|column| column.count > 0) {
            self.render_machines(&snapshot.machines);
            if self.progress.is_hidden() {
                writeln!(output, "{}", self.last_machine_line)?;
            }
            self.machine_bar.finish();
            // indicatif leaves the cursor at the right margin without a newline.
            // End the command's output explicitly before the shell renders its prompt.
            if !self.progress.is_hidden() {
                writeln!(output)?;
                output.flush()?;
            }
        } else {
            self.machine_bar.finish_and_clear();
        }
        Ok(())
    }

    fn render_summary(&mut self, summary: &str) {
        if summary != self.last_summary {
            self.summary_bar.set_message(summary.to_string());
            self.last_summary = summary.to_string();
        }
    }

    fn render_activities(&mut self, activities: &[ActivitySnapshot]) {
        let mut seen = HashMap::new();

        let mut visible: Vec<_> = activities
            .iter()
            .filter(|activity| activity.visible)
            .collect();
        visible.sort_by_key(|activity| activity.id);
        for activity in visible {
            seen.insert(activity.id, ());
            let bar = self.activity_bars.entry(activity.id).or_insert_with(|| {
                let bar = self
                    .progress
                    .insert_before(&self.summary_bar, ProgressBar::new_spinner());
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

    fn set_host_width<'a>(&mut self, hosts: impl Iterator<Item = &'a str>) {
        self.host_width = hosts.map(|host| host.chars().count()).max().unwrap_or(0);
    }

    fn log(&self, level: Level, scope: &EventScope, message: &str) {
        let host = scope.host.as_deref().unwrap_or(module_path!());
        let host = format!(
            "{host:<width$}",
            width = self.host_width.max(module_path!().len())
        );
        let stage = match scope.phase {
            Some(phase) => {
                let name = format!("{:<8}", phase.name());
                if palette::colors_enabled() {
                    palette::muted_stage(&name, phase)
                } else {
                    name
                }
            }
            None => " ".repeat(8),
        };
        let target = format!("{host} {stage}");
        self.progress.suspend(|| {
            for line in message.lines() {
                let level = if level == Level::Info { output_level(line) } else { level };
                log::log!(target: &target, level, "{line}");
            }
        });
    }

    fn emit_artifact(&self, artifact: &ArtifactEvent) {
        self.progress.suspend(|| match artifact {
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
        let mut refresh = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            let envelope = tokio::select! {
                envelope = events.recv() => match envelope { Some(envelope) => envelope, None => break },
                _ = refresh.tick() => {
                    self.render(self.reducer.snapshot());
                    continue;
                }
            };
            match &envelope.event {
                Event::Pipeline(PipelineEvent::Planned { stages }) => {
                    self.set_host_width(stages.iter().map(|(host, _)| host.as_str()));
                }
                Event::Pipeline(PipelineEvent::Started { hosts }) => {
                    self.set_host_width(hosts.iter().map(String::as_str));
                }
                _ => {}
            }
            self.reducer.apply(&envelope);

            crate::with_log_time(envelope.log_time.as_deref(), || match &envelope.event {
                Event::Log(log) => self.log(log.level, &envelope.scope, &log.message),
                Event::Artifact(artifact) => self.emit_artifact(artifact),
                Event::Output { bytes, .. } => {
                    for line in String::from_utf8_lossy(bytes).lines() {
                        self.log(Level::Info, &envelope.scope, line);
                    }
                }
                Event::Pipeline(PipelineEvent::Finished { .. }) => {}
                Event::Host(HostEvent::Finished { .. }) => {}
                _ => {}
            });

            let snapshot = self.reducer.snapshot();
            self.render(snapshot);
        }

        self.finish(&mut std::io::stderr())
    }
}

fn output_level(line: &str) -> Level {
    let plain = without_ansi(line);
    let line = plain.as_ref();
    let fingerprint = line.starts_with("Host key fingerprint is: ");
    let art_border = line.len() == 19
        && line.starts_with('+') && line.ends_with('+')
        && line.contains('[') && line.contains(']');
    let art_row = line.len() == 19 && line.starts_with('|') && line.ends_with('|')
        && line[1..18].bytes().all(|byte| b" .o+=*BOX@%&#/^SE".contains(&byte));
    let derivation_heading = line == "this derivation will be built:"
        || line.strip_prefix("these ")
            .and_then(|rest| rest.strip_suffix(" derivations will be built:"))
            .is_some_and(|count| !count.is_empty() && count.bytes().all(|byte| byte.is_ascii_digit()));
    let derivation_path = line.starts_with("  /nix/store/")
        && line.ends_with(".drv") && !line.trim().contains(char::is_whitespace);
    let copying_path = line.starts_with("copying path '/nix/store/")
        && line.ends_with("'...")
        && (line.contains("' from '") || line.contains("' to '"));
    if fingerprint || art_border || art_row || derivation_heading || derivation_path || copying_path {
        Level::Debug
    } else {
        Level::Info
    }
}

// Classify the visible text while preserving the original colors in the log.
fn without_ansi(line: &str) -> std::borrow::Cow<'_, str> {
    if !line.contains('\x1b') {
        return std::borrow::Cow::Borrowed(line);
    }
    let mut plain = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for ch in chars.by_ref() {
                if ('@'..='~').contains(&ch) { break; }
            }
        } else {
            plain.push(ch);
        }
    }
    std::borrow::Cow::Owned(plain)
}

fn summary_style() -> ProgressStyle {
    ProgressStyle::with_template("{msg}").unwrap_or_else(|_| ProgressStyle::default_spinner())
}

fn activity_style() -> ProgressStyle {
    ProgressStyle::with_template(&format!("{{spinner:.{}}} {{msg}}", palette::SPINNER_STYLE))
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
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

struct MachineDisplay {
    style: ProgressStyle,
    labels: String,
    plain: String,
}

fn machine_display(columns: &[MachineColumn]) -> MachineDisplay {
    let keys = ["m0", "m1", "m2", "m3", "m4", "m5"];
    let template = format!("{{spinner:.{}}} ", palette::SPINNER_STYLE)
        + &columns
            .iter()
            .zip(keys)
            .map(|(column, key)| {
                let color = if column.count == 0 {
                    palette::MUTED_STYLE.to_string()
                } else {
                    column.color.to_string()
                };
                format!("{{{key}:.{color}}}")
            })
            .collect::<Vec<_>>()
            .join(" / ")
        + &format!("  {{wide_msg:.{}}}", palette::MUTED_STYLE);
    let mut style = ProgressStyle::with_template(&template).expect("valid machine status template");
    for (column, key) in columns.iter().zip(keys) {
        let count = column.count;
        style = style.with_key(
            key,
            move |_: &indicatif::ProgressState, writer: &mut dyn std::fmt::Write| {
                let _ = write!(writer, "{count}");
            },
        );
    }
    let plain_labels = format!(
        "# {}",
        columns
            .iter()
            .map(|column| column.label)
            .collect::<Vec<_>>()
            .join("/")
    );
    // Carry label colors inside wide_msg so indicatif still truncates the legend.
    // Both labels and counts use the palette stored on MachineColumn.
    let colors = palette::colors_enabled();
    let labels = if colors {
        let colored = columns
            .iter()
            .map(|column| palette::regular(column.label, column.color))
            .collect::<Vec<_>>()
            .join("/");
        palette::muted(&format!("# {colored}"))
    } else {
        plain_labels.clone()
    };
    let counts = columns
        .iter()
        .map(|column| column.count.to_string())
        .collect::<Vec<_>>()
        .join(" / ");
    let plain = format!("{counts}  {plain_labels}");
    MachineDisplay {
        style,
        labels,
        plain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventEnvelope, PipelinePhase};
    use indicatif::{ProgressDrawTarget, TermLike};
    use std::{
        io,
        sync::{Arc, Mutex},
    };

    #[derive(Clone, Debug)]
    struct RecordingTerminal {
        width: u16,
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    impl Write for RecordingTerminal {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl TermLike for RecordingTerminal {
        fn width(&self) -> u16 {
            self.width
        }
        fn height(&self) -> u16 {
            47
        }
        fn move_cursor_up(&self, n: usize) -> io::Result<()> {
            if n == 0 {
                return Ok(());
            }
            self.write_str(&format!("\x1b[{n}A"))
        }
        fn move_cursor_down(&self, n: usize) -> io::Result<()> {
            if n == 0 {
                return Ok(());
            }
            self.write_str(&format!("\x1b[{n}B"))
        }
        fn move_cursor_right(&self, n: usize) -> io::Result<()> {
            if n == 0 {
                return Ok(());
            }
            self.write_str(&format!("\x1b[{n}C"))
        }
        fn move_cursor_left(&self, n: usize) -> io::Result<()> {
            if n == 0 {
                return Ok(());
            }
            self.write_str(&format!("\x1b[{n}D"))
        }
        fn write_line(&self, text: &str) -> io::Result<()> {
            self.write_str(&format!("{text}\n"))
        }
        fn write_str(&self, text: &str) -> io::Result<()> {
            self.bytes
                .lock()
                .unwrap()
                .extend_from_slice(text.as_bytes());
            Ok(())
        }
        fn clear_line(&self) -> io::Result<()> {
            self.write_str("\r\x1b[2K")
        }
        fn flush(&self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn copying_path_progress_is_debug_output() {
        for line in [
            "copying path '/nix/store/example-grammar' from 'https://cache.nixos.org'...",
            "copying path '/nix/store/example-grammar' to 'ssh://root@host'...",
            "copying path '\x1b[35;1m/nix/store/example-grammar\x1b[0m' from 'https://cache.nixos.org'...",
        ] {
            assert_eq!(output_level(line), Level::Debug, "{line}");
        }
        for line in ["error: copying path '/nix/store/example' failed",
            "copying path '/nix/store/example' from 'https://cache.nixos.org': connection failed"] {
            assert_eq!(output_level(line), Level::Info, "{line}");
        }
    }

    #[test]
    fn derivation_plan_details_are_debug_output() {
        for line in ["these 13 derivations will be built:", "this derivation will be built:",
            "these \x1b[35;1m15\x1b[0m derivations will be built:",
            "these \x1b[35;1m13\x1b[0m derivations will be built:",
            "  \x1b[35;1m/nix/store/example.drv\x1b[0m",
            "  /nix/store/nnvbgzc0cnv6ypiap2rnww9hpmkp06a8-system-path.drv"] {
            assert_eq!(output_level(line), Level::Debug, "{line}");
        }
        for line in ["error: failed to build /nix/store/example.drv",
            "building '/nix/store/example.drv'...", "  /nix/store/example.drv failed"] {
            assert_eq!(output_level(line), Level::Info, "{line}");
        }
    }

    #[test]
    fn ssh_fingerprints_and_randomart_are_debug_output() {
        for line in [
            "Host key fingerprint is: SHA256:example",
            "+--[ED25519 256]--+",
            "|         .+.     |",
            "|    . = So.* o . |",
            "+----[SHA256]-----+",
        ] {
            assert_eq!(output_level(line), Level::Debug, "{line}");
        }
        for line in ["activating the configuration...", "Host key verification failed.",
            "WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!", "| service status  |"] {
            assert_eq!(output_level(line), Level::Info, "{line}");
        }
    }

    #[test]
    fn failures_are_grouped_before_final_status_without_duplicate_deploy_failures() {
        let mut reporter = IndicatifReporter::new();
        reporter.progress.set_draw_target(ProgressDrawTarget::hidden());
        for (host, phase) in [("zeta", PipelinePhase::Build), ("alpha", PipelinePhase::Build),
            ("beta", PipelinePhase::Activate), ("beta", PipelinePhase::Deploy)] {
            reporter.reducer.apply(&EventEnvelope {
                log_time: None, scope: Default::default(),
                event: Event::Host(HostEvent::Finished { host: host.into(), phase, success: false }),
            });
        }
        let mut output = Vec::new();
        reporter.finish(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        let lines: Vec<_> = output.lines().collect();
        assert_eq!(lines[0], "2 machines failed to build: alpha, zeta");
        assert_eq!(lines[1], "1 machine failed to deploy: beta");
        assert_eq!(lines.len(), 3);
        assert!(lines[2].contains("# failed/"));
    }

    #[test]
    fn terminal_completion_ends_with_a_newline_for_every_outcome() {
        for width in [80, 175] {
            for outcome in [Some(true), Some(false), None] {
                let terminal = RecordingTerminal {
                    width,
                    bytes: Arc::default(),
                };
                let mut reporter = IndicatifReporter::new();
                reporter
                    .progress
                    .set_draw_target(ProgressDrawTarget::term_like(Box::new(terminal.clone())));
                assert!(!reporter.progress.is_hidden());
                let mut events = vec![
                    Event::Pipeline(PipelineEvent::Planned {
                        stages: vec![("example".into(), vec![PipelinePhase::Build])],
                    }),
                    Event::Host(HostEvent::Finished {
                        host: "example".into(),
                        phase: PipelinePhase::Build,
                        success: outcome != Some(false),
                    }),
                ];
                if let Some(success) = outcome {
                    events.push(Event::Pipeline(PipelineEvent::Finished { success }));
                }
                for event in events {
                    reporter.reducer.apply(&EventEnvelope {
                        log_time: None,
                        scope: Default::default(),
                        event,
                    });
                }
                reporter.render(reporter.reducer.snapshot());
                let running_line = reporter.last_machine_line.clone();
                let running_labels = reporter.machine_bar.message();
                reporter.finish(&mut terminal.clone()).unwrap();
                assert_eq!(reporter.last_machine_line, running_line);
                assert_eq!(reporter.machine_bar.message(), running_labels);
                assert!(reporter.machine_bar.prefix().is_empty());
                drop(reporter);
                let bytes = terminal.bytes.lock().unwrap();
                let text = String::from_utf8_lossy(&bytes);
                assert!(
                    text.contains("failed") && text.contains("build") && text.contains("done"),
                    "missing final status at width {width}"
                );
                assert!(!text.contains('✓') && !text.contains('✗') && !text.contains('■'));
                assert!(
                    bytes.ends_with(b"\n"),
                    "terminal output lacks a final newline at width {width}: {text:?}"
                );
            }
        }
    }
}
