#[cfg(unix)]
#[test]
fn failed_build_replays_raw_input_without_executing_commands() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("flake.nix"), "{}").unwrap();
    let nix = dir.path().join("fake-nix");
    fs::write(&nix, "#!/bin/sh\nprintf 'captured stdout'\nprintf '@nix {\"action\":\"msg\",\"level\":3,\"msg\":\"raw diagnostic\"}\\n' >&2\nexit 7\n").unwrap();
    fs::set_permissions(&nix, fs::Permissions::from_mode(0o700)).unwrap();
    let data_home = dir.path().join("data");
    let output = Command::new(env!("CARGO_BIN_EXE_wat"))
        .current_dir(dir.path())
        .env("WAT_NIX_BINARY", &nix)
        .env("XDG_DATA_HOME", &data_home)
        .args(["build", "example"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let recording = fs::read_dir(dir.path().join(".wat/logs"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(!data_home.exists());
    assert_eq!(
        fs::read_to_string(dir.path().join(".wat/.gitignore")).unwrap(),
        "/logs/\n"
    );
    let entries: Vec<serde_json::Value> = fs::read_to_string(&recording)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(entries[0]["version"], 1);
    assert!(
        entries
            .iter()
            .any(|entry| entry["kind"] == "exit" && entry["code"] == 7)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry["kind"] == "output" && entry["stream"] == "stdout")
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry["kind"] == "output" && entry["scope"]["host"] == "example")
    );
    fs::remove_file(nix).unwrap();
    fs::remove_file(dir.path().join("flake.nix")).unwrap();
    let replay = Command::new(env!("CARGO_BIN_EXE_wat"))
        .current_dir(dir.path())
        .env("WAT_NIX_BINARY", "/nonexistent/nix")
        .env("XDG_DATA_HOME", &data_home)
        .args(["replay", recording.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!replay.status.success());
    let stderr = String::from_utf8_lossy(&replay.stderr);
    assert!(stderr.contains("raw diagnostic"), "{stderr}");
    assert!(stderr.contains("failed to build"), "{stderr}");
    assert!(!stderr.contains("Recording:"), "{stderr}");
    assert!(!String::from_utf8_lossy(&replay.stdout).contains("captured stdout"));
    assert_eq!(
        fs::read_dir(dir.path().join(".wat/logs")).unwrap().count(),
        1
    );
}

#[test]
fn replay_joins_output_chunks_and_finishes_partial_lines() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("output.jsonl");
    let mut entries = vec![
        json!({"version": 1}),
        json!({"kind": "event", "elapsed_us": 0, "envelope": {
            "scope": {"host": null, "phase": null},
            "event": {"Pipeline": {"Started": {"hosts": ["sirrah", "longer-host"]}}}
        }}),
    ];
    for (command, stream, bytes) in [
        (1, "stderr", b"Checking switch inhibitors...".as_slice()),
        (2, "stdout", b"other host\n".as_slice()),
        (1, "stderr", b" done\ntrailing error".as_slice()),
        (1, "stdout", b"trailing output".as_slice()),
    ] {
        entries.push(
            json!({"kind": "output", "elapsed_us": 0, "command": command,
            "stream": stream, "bytes": bytes, "lix": false}),
        );
    }
    for entry in &mut entries {
        if entry["command"] == 1 {
            entry["scope"] = json!({"host": "sirrah", "phase": "Deploy"});
        }
    }
    entries.push(json!({"kind": "exit", "elapsed_us": 0, "command": 1, "success": true}));
    std::fs::write(
        &path,
        entries
            .iter()
            .map(|entry| format!("{entry}\n"))
            .collect::<String>(),
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_wat"))
        .arg("replay")
        .arg(&path)
        .env("RUST_LOG", "info")
        .env("RUST_LOG_STYLE", "never")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let lines: Vec<_> = stderr.lines().collect();
    assert_eq!(lines.len(), 4, "{stderr}");
    for (line, expected) in lines.iter().zip([
        "other host",
        "Checking switch inhibitors... done",
        "trailing output",
        "trailing error",
    ]) {
        assert!(line.starts_with('['), "missing timestamp: {line}");
        assert!(line.contains(" INFO] "), "missing log level: {line}");
        assert!(line.ends_with(expected), "{line}");
        let (host, stage) = if expected == "other host" {
            ("wat::reporter", "")
        } else {
            ("sirrah", "deploy")
        };
        assert!(line.contains(&format!("Z {host:<13} {stage:<8}  INFO]")), "{line}");
    }

    let filtered = std::process::Command::new(env!("CARGO_BIN_EXE_wat"))
        .arg("replay")
        .arg(&path)
        .env("RUST_LOG", "warn")
        .output()
        .unwrap();
    assert!(filtered.status.success());
    assert!(filtered.stdout.is_empty());
    assert!(filtered.stderr.is_empty());

    // Legacy recordings stored an empty scope for SSH output. Recover the host
    // from the command while retaining line buffering and hostname padding.
    for entry in &mut entries {
        if entry["command"] == 1 && entry["kind"] == "output" {
            entry["scope"] = json!({"host": null, "phase": null});
        }
    }
    entries.insert(1, json!({"kind": "command", "elapsed_us": 0,
        "command": 1, "program": "ssh", "args": ["root@sirrah", "activate"]}));
    std::fs::write(&path, entries.iter().map(|entry| format!("{entry}\n")).collect::<String>()).unwrap();
    let legacy = std::process::Command::new(env!("CARGO_BIN_EXE_wat"))
        .arg("replay").arg(&path)
        .env("RUST_LOG", "info").env("RUST_LOG_STYLE", "never")
        .output().unwrap();
    assert!(legacy.status.success());
    let legacy_stderr = String::from_utf8(legacy.stderr).unwrap();
    let messages = |text: &str| text.lines().map(|line| line.split_once("Z ").unwrap().1.to_string()).collect::<Vec<_>>();
    assert_eq!(messages(&legacy_stderr), messages(&stderr));
}

#[test]
fn replay_keeps_final_machine_outcome_in_redirected_output() {
    use serde_json::json;
    for success in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("final.jsonl");
        let events = [
            json!({"Pipeline": {"Planned": {"stages": [["example", ["Evaluate", "Build"]]]}}}),
            json!({"Host": {"Finished": {"host": "example", "phase": "Build", "success": success}}}),
            json!({"Pipeline": {"Finished": {"success": success}}}),
        ];
        let mut recording = String::from("{\"version\":1}\n");
        for event in events {
            recording.push_str(&format!(
                "{}\n",
                json!({"kind": "event", "elapsed_us": 0, "envelope": {"scope": {}, "event": event}})
            ));
        }
        std::fs::write(&path, recording).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_wat"))
            .arg("replay")
            .arg(path)
            .output()
            .unwrap();
        assert!(output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        let expected = if success {
            "0 / 0 / 0 / 1  # failed/eval/build/done"
        } else {
            "1 / 0 / 0 / 0  # failed/eval/build/done"
        };
        assert!(stderr.ends_with(&format!("{expected}\n")), "{stderr}");
    }
}
