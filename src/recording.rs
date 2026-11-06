//! Versioned JSONL input capture. Raw Lix input is replayed through the current adapter.
use crate::{
    backend::lix_parser::LixEventAdapter,
    event::{Event, EventBus, EventEnvelope, EventScope, EventSender, HostEvent, PipelineEvent, PipelinePhase, LogEvent},
    reporter::{IndicatifReporter, Reporter},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Instant,
};

tokio::task_local! {
    static REPLAY_LOG_TIME: std::cell::RefCell<Option<String>>;
}

pub(crate) fn replay_log_time() -> Option<String> {
    REPLAY_LOG_TIME.try_with(|time| time.borrow().clone()).ok().flatten()
}

struct Recording {
    file: File,
    start: Instant,
}
static RECORDING: OnceLock<Mutex<Recording>> = OnceLock::new();

pub fn init() -> Result<()> {
    init_in(&std::env::current_dir().context("failed to locate working directory")?)?;
    Ok(())
}

fn prepare_directory(repository: &Path) -> Result<PathBuf> {
    let directory = repository.join(".wat");
    let logs = directory.join("logs");
    std::fs::create_dir_all(&logs)
        .with_context(|| format!("failed to create recording directory {}", logs.display()))?;
    let ignore_path = directory.join(".gitignore");
    let contents = match std::fs::read_to_string(&ignore_path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error).context("failed to read .wat/.gitignore"),
    };
    if !contents.lines().any(|line| line.trim() == "/logs/") {
        let mut ignore = File::options()
            .create(true)
            .append(true)
            .open(&ignore_path)
            .context("failed to open .wat/.gitignore")?;
        if !contents.is_empty() && !contents.ends_with('\n') {
            writeln!(ignore)?;
        }
        writeln!(ignore, "/logs/").context("failed to update .wat/.gitignore")?;
    }
    Ok(logs)
}

fn init_in(repository: &Path) -> Result<PathBuf> {
    let directory = prepare_directory(repository)?;
    let (mut file, path) = tempfile::Builder::new()
        .prefix("wat-events-")
        .suffix(".jsonl")
        .tempfile_in(&directory)
        .context("failed to create recording")?
        .keep()
        .context("failed to retain recording")?;
    let start = Instant::now();
    let started_at = jiff::Timestamp::now();
    writeln!(file, "{}", json!({"version": 1, "started_at": started_at.to_string()}))?;
    RECORDING
        .set(Mutex::new(Recording {
            file,
            start,
        }))
        .map_err(|_| anyhow::anyhow!("recording already initialized"))?;
    eprintln!("Recording: {}", path.display());
    Ok(path)
}

pub fn write(mut value: Value) {
    let Some(recording) = RECORDING.get() else {
        return;
    };
    let mut recording = recording.lock().unwrap_or_else(|error| error.into_inner());
    value["elapsed_us"] = json!(recording.start.elapsed().as_micros() as u64);
    if let Err(error) = writeln!(recording.file, "{value}") {
        eprintln!("failed to write recording: {error}");
    }
}

pub fn raw(scope: &EventScope, command: u64, stream: &str, bytes: &[u8], lix: bool) {
    write(
        json!({"kind": "output", "scope": scope, "command": command, "stream": stream, "bytes": bytes, "lix": lix}),
    );
}

pub fn start(command: &tokio::process::Command) -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    write(
        json!({"kind": "command", "command": id, "program": command.as_std().get_program().to_string_lossy(), "args": command.as_std().get_args().map(|arg| arg.to_string_lossy()).collect::<Vec<_>>() }),
    );
    id
}

// An interrupted writer may leave its final JSONL entry unfinished. Only ignore
// JSON EOF at the end of the file; other parse and I/O errors remain errors.
fn recording_values<'a>(
    lines: impl Iterator<Item = std::io::Result<String>> + 'a,
    truncated: &'a mut bool,
) -> impl Iterator<Item = Result<Value>> + 'a {
    let mut lines = lines.peekable();
    std::iter::from_fn(move || {
        let line = match lines.next()? {
            Ok(line) => line,
            Err(error) => return Some(Err(error.into())),
        };
        match serde_json::from_str(&line) {
            Ok(value) => Some(Ok(value)),
            Err(error) if error.is_eof() && lines.peek().is_none() => {
                *truncated = true;
                None
            }
            Err(error) => Some(Err(error.into())),
        }
    })
}

pub async fn replay(path: &Path) -> Result<()> {
    let mut lines = BufReader::new(File::open(path).context("failed to open recording")?).lines();
    let header: Value = serde_json::from_str(&lines.next().context("empty recording")??)?;
    if header["version"] != 1 {
        bail!("unsupported recording version");
    }
    let started_at = header.get("started_at")
        .map(|value| value.as_str().context("invalid recording start time")?
            .parse::<jiff::Timestamp>().context("invalid recording start time"))
        .transpose()?;
    let mut stages = std::collections::BTreeMap::<String, Vec<crate::event::PipelinePhase>>::new();
    let mut planned = None;
    let mut copies = std::collections::HashMap::new();
    let mut command_hosts = std::collections::HashMap::new();
    let mut activation_host = None;
    let mut activation_commands = std::collections::HashMap::new();
    let mut service_error_hosts = std::collections::HashSet::new();
    let mut truncated = false;
    for value in recording_values(BufReader::new(File::open(path)?).lines().skip(1), &mut truncated) {
        let value = value?;
        if let Some((command, host)) = legacy_ssh_command(&value) {
            command_hosts.insert(
                command,
                EventScope {
                    host: Some(host),
                    phase: Some(crate::event::PipelinePhase::Deploy),
                },
            );
        }
        if let Some((command, host)) = legacy_copy_command(&value) {
            command_hosts.insert(
                command,
                EventScope {
                    host: Some(host.clone()),
                    phase: Some(crate::event::PipelinePhase::Upload),
                },
            );
            copies.insert(command, host);
        }
        if value["kind"] == "command" {
            let is_activation = value["args"].as_array().is_some_and(|args| {
                args.iter().filter_map(Value::as_str).any(|arg| arg.contains("/bin/switch-to-configuration"))
            });
            if is_activation {
                let host = legacy_ssh_command(&value).map(|(_, host)| host)
                    .or_else(|| activation_host.clone());
                if let (Some(id), Some(host)) = (value["command"].as_u64(), host) {
                    activation_commands.insert(id, host);
                }
            }
        }
        if value["kind"] == "exit" && value["code"] == 4 {
            if let Some(host) = value["command"].as_u64().and_then(|id| activation_commands.get(&id)) {
                service_error_hosts.insert(host.clone());
            }
        }
        if value["kind"] != "event" {
            continue;
        }
        let envelope: EventEnvelope = serde_json::from_value(value["envelope"].clone())?;
        if let Event::Host(HostEvent::Started { host, phase: PipelinePhase::Activate }) = &envelope.event {
            activation_host = Some(host.clone());
        }
        match envelope.event {
            Event::Pipeline(crate::event::PipelineEvent::Planned { stages }) => {
                planned = Some(stages)
            }
            Event::Host(crate::event::HostEvent::Started { host, phase })
            | Event::Host(crate::event::HostEvent::Finished { host, phase, .. }) => {
                let phases = stages.entry(host).or_default();
                if !phases.contains(&phase) {
                    phases.push(phase);
                }
            }
            _ => {}
        }
    }
    // New recordings provide explicit stages; infer copy boundaries only for legacy input.
    if planned.is_some() {
        copies.clear();
    }
    for host in copies.values() {
        if let Some(phases) = stages.get_mut(host) {
            if !phases.contains(&crate::event::PipelinePhase::Upload) {
                let position = phases
                    .iter()
                    .position(|phase| *phase == crate::event::PipelinePhase::Deploy)
                    .unwrap_or(phases.len());
                phases.insert(position, crate::event::PipelinePhase::Upload);
            }
        }
    }
    let stages = planned.unwrap_or_else(|| stages.into_iter().collect());
    let (bus, events) = EventBus::new(512);
    let reporter = tokio::spawn(async move { IndicatifReporter::new().run(events).await });
    bus.sender()
        .unrecorded()
        .send(Event::Pipeline(crate::event::PipelineEvent::Planned {
            stages,
        }))
        .await?;
    let result = REPLAY_LOG_TIME.scope(std::cell::RefCell::new(None), async {
        let mut adapters = std::collections::HashMap::new();
        let mut previous = 0;
        let mut failed_hosts = std::collections::HashSet::new();
        let mut output = std::collections::HashMap::<(u64, bool), (EventSender, Vec<u8>)>::new();
        let mut replay_truncated = false;
        for value in recording_values(lines, &mut replay_truncated) {
            let value = value?;
            let elapsed = value["elapsed_us"]
                .as_u64()
                .context("missing recording timestamp")?;
            if elapsed > previous {
                tokio::time::sleep(std::time::Duration::from_micros(elapsed - previous)).await;
            }
            previous = elapsed;
            if let Some(start) = started_at {
                let micros = i64::try_from(elapsed).context("recording timestamp out of range")?;
                let timestamp = start.checked_add(std::time::Duration::from_micros(micros as u64))?;
                let time = timestamp.to_zoned(jiff::tz::TimeZone::system())
                    .strftime("%H:%M:%S").to_string();
                REPLAY_LOG_TIME.with(|current| *current.borrow_mut() = Some(time));
            }
            match value["kind"].as_str() {
                Some("event") => {
                    let mut envelope: EventEnvelope =
                        serde_json::from_value(value["envelope"].clone())?;
                    let sender = bus.sender().unrecorded().with_scope(envelope.scope);
                    match &mut envelope.event {
                        Event::Host(HostEvent::Finished { host, phase, success }) => {
                            if !*success && service_error_hosts.contains(host)
                                && matches!(phase, PipelinePhase::Activate | PipelinePhase::Deploy)
                            {
                                *success = true;
                                if *phase == PipelinePhase::Activate {
                                    sender.with_host(host.clone()).with_phase(*phase).send(Event::Log(LogEvent {
                                        level: log::Level::Warn,
                                        message: "Configuration activated, but activation reported service errors (exit status 4)".into(),
                                    })).await?;
                                }
                            }
                            if !*success { failed_hosts.insert(host.clone()); }
                        }
                        Event::Pipeline(PipelineEvent::Finished { success })
                        | Event::Pipeline(PipelineEvent::PhaseFinished { success, .. })
                            if !service_error_hosts.is_empty() => {
                                *success = failed_hosts.is_empty();
                            }
                        _ => {}
                    }
                    sender.send(envelope.event).await?;
                }
                Some("output") => {
                    let bytes: Vec<u8> = serde_json::from_value(value["bytes"].clone())?;
                    let mut scope: EventScope = value.get("scope")
                        .map(|scope| serde_json::from_value(scope.clone()))
                        .transpose()?
                        .unwrap_or_default();
                    if let Some(inferred) = value["command"].as_u64()
                        .and_then(|id| command_hosts.get(&id))
                    {
                        scope.host = scope.host.or_else(|| inferred.host.clone());
                        scope.phase = scope.phase.or(inferred.phase);
                    }
                    let sender = bus.sender().unrecorded().with_scope(scope);
                    if value["lix"] == true && value["stream"] == "stderr" {
                        let id = value["command"].as_u64().context("missing command id")?;
                        adapters
                            .entry(id)
                            .or_insert_with(|| LixEventAdapter::new(sender))
                            .handle_stderr_line(
                                String::from_utf8_lossy(&bytes).trim_end_matches('\n'),
                            )
                            .await?;
                    } else if value["stream"] == "captured_stdout"
                        || value["stream"] == "captured_stderr"
                        || (value["lix"] == true && value["stream"] == "stdout")
                    {
                        // Captured for inspection, but not displayed by the live run.
                    } else {
                        let id = value["command"].as_u64().context("missing command id")?;
                        let stderr = value["stream"] != "stdout";
                        let (sender, pending) = output
                            .entry((id, stderr))
                            .or_insert_with(|| (sender, Vec::new()));
                        pending.extend_from_slice(&bytes);
                        while let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
                            let bytes = pending.drain(..=end).collect();
                            sender.send(Event::Output { stderr, bytes })
                                .await?;
                        }
                    }
                }
                Some("outcome") => {
                    if let Some(error) = value["error"].as_str() {
                        if !failed_hosts.is_empty() {
                            return Err(crate::operations::PipelineFailed(error.to_owned()).into());
                        }
                        // Legacy outcomes contain the activation error that we just
                        // reclassified. Preserve any unrelated infrastructure errors.
                        let only_service_errors = !service_error_hosts.is_empty() && error.lines().all(|line| {
                            service_error_hosts.iter().any(|host| line == format!(
                                "failed to switch configuration remotely on {host}: ssh exited with status exit status: 4"
                            )) || line.starts_with("failed to run switch-to-configuration ")
                                && line.ends_with(": sudo exited with status exit status: 4")
                        });
                        if !only_service_errors { bail!("{error}"); }
                    }
                }
                Some("exit") => {
                    if let Some(host) = value["command"].as_u64().and_then(|id| copies.get(&id)) {
                        bus.sender()
                            .unrecorded()
                            .send(Event::Host(crate::event::HostEvent::Finished {
                                host: host.clone(),
                                phase: crate::event::PipelinePhase::Upload,
                                success: value["success"] == true,
                            }))
                            .await?;
                    }
                    if let Some(id) = value["command"].as_u64() {
                        for stderr in [false, true] {
                            if let Some((sender, bytes)) = output.remove(&(id, stderr)) {
                                if !bytes.is_empty() {
                                    sender.send(Event::Output { stderr, bytes })
                                        .await?;
                                }
                            }
                        }
                        if let Some(mut adapter) = adapters.remove(&id) {
                            adapter.finish(value["success"] == true);
                        }
                    }
                }
                Some("command") => {
                    if let Some(host) = value["command"].as_u64().and_then(|id| copies.get(&id)) {
                        bus.sender()
                            .unrecorded()
                            .send(Event::Host(crate::event::HostEvent::Started {
                                host: host.clone(),
                                phase: crate::event::PipelinePhase::Upload,
                            }))
                            .await?;
                    }
                }
                _ => bail!("unknown recording entry"),
            }
        }
        for ((_, stderr), (sender, bytes)) in output {
            if !bytes.is_empty() {
                sender.send(Event::Output { stderr, bytes })
                    .await?;
            }
        }
        if truncated {
            bus.sender().unrecorded().send(Event::Log(LogEvent {
                level: log::Level::Warn,
                message: "Ignoring incomplete final recording entry; showing last recorded state".into(),
            })).await?;
        }
        Ok(())
    })
    .await;
    drop(bus);
    reporter.await.context("reporter task failed")??;
    result
}

pub async fn execute_visible(
    command: &mut tokio::process::Command,
    events: &EventSender,
) -> Result<std::process::ExitStatus> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let id = start(command);
    let mut child = command.spawn().context("failed to start command")?;
    let mut stdout = BufReader::new(child.stdout.take().context("missing stdout")?);
    let mut stderr = BufReader::new(child.stderr.take().context("missing stderr")?);
    let out = async {
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            let count = stdout.read_until(b'\n', &mut buffer).await?;
            if count == 0 {
                break;
            }
            raw(
                events.scope(),
                id,
                "stdout",
                &buffer[..count],
                false,
            );
            events
                .unrecorded()
                .send(Event::Output {
                    stderr: false,
                    bytes: buffer.clone(),
                })
                .await?;
        }
        Ok::<_, anyhow::Error>(())
    };
    let err = async {
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            let count = stderr.read_until(b'\n', &mut buffer).await?;
            if count == 0 {
                break;
            }
            raw(
                events.scope(),
                id,
                "stderr",
                &buffer[..count],
                false,
            );
            events
                .unrecorded()
                .send(Event::Output {
                    stderr: true,
                    bytes: buffer.clone(),
                })
                .await?;
        }
        Ok::<_, anyhow::Error>(())
    };
    let (out, err, status) = tokio::join!(out, err, child.wait());
    let status = status?;
    write(
        json!({"kind": "exit", "command": id, "success": status.success(), "code": status.code()}),
    );
    out?;
    err?;
    Ok(status)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn creates_local_logs_and_preserves_ignore_entries() {
        let repository = tempfile::tempdir().unwrap();
        let logs = prepare_directory(repository.path()).unwrap();
        assert_eq!(logs, repository.path().join(".wat/logs"));
        assert!(logs.is_dir());
        let ignore = repository.path().join(".wat/.gitignore");
        assert_eq!(std::fs::read_to_string(&ignore).unwrap(), "/logs/\n");
        std::fs::write(&ignore, "# existing rules\ncache/").unwrap();
        prepare_directory(repository.path()).unwrap();
        let expected = "# existing rules\ncache/\n/logs/\n";
        assert_eq!(std::fs::read_to_string(&ignore).unwrap(), expected);
        prepare_directory(repository.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&ignore).unwrap(), expected);
    }

    #[tokio::test]
    async fn captures_both_streams_and_exit_status() {
        let directory = tempfile::tempdir().unwrap();
        let path = init_in(directory.path()).unwrap();
        let mut command = tokio::process::Command::new("sh");
        command.args([
            "-c",
            "printf 'activation output'; printf 'activation error' >&2; exit 9",
        ]);
        let (bus, mut output) = EventBus::new(16);
        assert_eq!(
            execute_visible(&mut command, &bus.sender().with_host("example"))
                .await
                .unwrap()
                .code(),
            Some(9)
        );
        drop(bus);
        let mut displayed = Vec::new();
        while let Some(envelope) = output.recv().await {
            assert_eq!(envelope.scope.host.as_deref(), Some("example"));
            if let Event::Output { stderr, bytes } = envelope.event {
                displayed.push((stderr, bytes));
            }
        }
        assert!(displayed.contains(&(false, b"activation output".to_vec())));
        assert!(displayed.contains(&(true, b"activation error".to_vec())));
        let entries: Vec<Value> = std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        for entry in entries.iter().filter(|entry| entry["kind"] == "output") {
            assert_eq!(entry["scope"]["host"], "example");
        }
        for (stream, expected) in [
            ("stdout", "activation output"),
            ("stderr", "activation error"),
        ] {
            let bytes: Vec<u8> = entries
                .iter()
                .filter(|entry| entry["stream"] == stream)
                .flat_map(|entry| {
                    serde_json::from_value::<Vec<u8>>(entry["bytes"].clone()).unwrap()
                })
                .collect();
            assert_eq!(bytes, expected.as_bytes());
        }
        assert!(
            entries
                .iter()
                .any(|entry| entry["kind"] == "exit" && entry["code"] == 9)
        );
    }
}

// Restrict inference to the remote deployment copy shape used by wat.
fn legacy_copy_command(value: &Value) -> Option<(u64, String)> {
    if value["kind"] != "command" {
        return None;
    }
    let program = Path::new(value["program"].as_str()?)
        .file_name()?
        .to_str()?;
    if !matches!(program, "nix" | "lix") {
        return None;
    }
    let args = value["args"].as_array()?;
    if !args.iter().any(|arg| arg == "copy") {
        return None;
    }
    let target = args.windows(2).find(|pair| pair[0] == "--to")?[1].as_str()?;
    let host = target.strip_prefix("ssh://root@")?;
    if host.is_empty() {
        return None;
    }
    Some((value["command"].as_u64()?, host.to_string()))
}

// Older recordings omitted scope for visible SSH output. Only recognize wat's
// direct root@host invocation, rather than guessing at arbitrary SSH options.
fn legacy_ssh_command(value: &Value) -> Option<(u64, String)> {
    if value["kind"] != "command"
        || Path::new(value["program"].as_str()?).file_name()? != "ssh"
    {
        return None;
    }
    let args = value["args"].as_array()?;
    if args.len() != 2 {
        return None;
    }
    let host = args[0].as_str()?.strip_prefix("root@")?;
    if host.is_empty() {
        return None;
    }
    Some((value["command"].as_u64()?, host.to_string()))
}
