//! Versioned JSONL input capture. Raw Lix input is replayed through the current adapter.
use crate::{
    backend::lix_parser::LixEventAdapter,
    event::{EventBus, EventEnvelope, EventScope},
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
    writeln!(file, "{}", json!({"version": 1}))?;
    RECORDING
        .set(Mutex::new(Recording {
            file,
            start: Instant::now(),
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

pub async fn replay(path: &Path) -> Result<()> {
    let mut lines = BufReader::new(File::open(path).context("failed to open recording")?).lines();
    let header: Value = serde_json::from_str(&lines.next().context("empty recording")??)?;
    if header["version"] != 1 {
        bail!("unsupported recording version");
    }
    let (bus, events) = EventBus::new(512);
    let reporter = tokio::spawn(async move { IndicatifReporter::new().run(events).await });
    let result = async {
        let mut adapters = std::collections::HashMap::new();
        let mut previous = 0;
        for line in lines {
            let value: Value = serde_json::from_str(&line?)?;
            let elapsed = value["elapsed_us"]
                .as_u64()
                .context("missing recording timestamp")?;
            tokio::time::sleep(std::time::Duration::from_micros(
                elapsed.saturating_sub(previous),
            ))
            .await;
            previous = elapsed;
            match value["kind"].as_str() {
                Some("event") => {
                    let envelope: EventEnvelope =
                        serde_json::from_value(value["envelope"].clone())?;
                    let mut sender = bus.sender().unrecorded();
                    if let Some(host) = envelope.scope.host {
                        sender = sender.with_host(host);
                    }
                    if let Some(phase) = envelope.scope.phase {
                        sender = sender.with_phase(phase);
                    }
                    sender.send(envelope.event).await?;
                }
                Some("output") => {
                    let bytes: Vec<u8> = serde_json::from_value(value["bytes"].clone())?;
                    if value["lix"] == true && value["stream"] == "stderr" {
                        let scope: EventScope = serde_json::from_value(value["scope"].clone())?;
                        let mut sender = bus.sender().unrecorded();
                        if let Some(host) = scope.host {
                            sender = sender.with_host(host);
                        }
                        if let Some(phase) = scope.phase {
                            sender = sender.with_phase(phase);
                        }
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
                    } else if value["stream"] == "stdout" {
                        std::io::stdout().write_all(&bytes)?;
                    } else {
                        std::io::stderr().write_all(&bytes)?;
                    }
                }
                Some("outcome") => {
                    if let Some(error) = value["error"].as_str() {
                        bail!("{error}");
                    }
                }
                Some("exit") => {
                    if let Some(id) = value["command"].as_u64() {
                        if let Some(mut adapter) = adapters.remove(&id) {
                            adapter.finish(value["success"] == true);
                        }
                    }
                }
                Some("command") => {}
                _ => bail!("unknown recording entry"),
            }
        }
        Ok(())
    }
    .await;
    drop(bus);
    reporter.await.context("reporter task failed")??;
    result
}

pub async fn execute_visible(
    command: &mut tokio::process::Command,
) -> Result<std::process::ExitStatus> {
    use tokio::io::AsyncReadExt;
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let id = start(command);
    let mut child = command.spawn().context("failed to start command")?;
    let mut stdout = child.stdout.take().context("missing stdout")?;
    let mut stderr = child.stderr.take().context("missing stderr")?;
    let out = async {
        let mut buffer = [0; 8192];
        loop {
            let count = stdout.read(&mut buffer).await?;
            if count == 0 {
                break;
            }
            raw(
                &EventScope::default(),
                id,
                "stdout",
                &buffer[..count],
                false,
            );
            std::io::stdout().write_all(&buffer[..count])?;
        }
        Ok::<_, std::io::Error>(())
    };
    let err = async {
        let mut buffer = [0; 8192];
        loop {
            let count = stderr.read(&mut buffer).await?;
            if count == 0 {
                break;
            }
            raw(
                &EventScope::default(),
                id,
                "stderr",
                &buffer[..count],
                false,
            );
            std::io::stderr().write_all(&buffer[..count])?;
        }
        Ok::<_, std::io::Error>(())
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
        assert_eq!(execute_visible(&mut command).await.unwrap().code(), Some(9));
        let entries: Vec<Value> = std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
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
