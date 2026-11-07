#[cfg(unix)]
#[test]
fn failed_evaluation_replays_raw_input_without_executing_commands() {
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
    assert!(entries[0]["started_at"].as_str().unwrap().parse::<jiff::Timestamp>().is_ok());
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
    assert!(stderr.contains("1 machine failed to evaluate: example"), "{stderr}");
    assert!(!stderr.contains("exited with status"), "{stderr}");
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
        assert!(line.contains(&format!(" {host:<13} {stage:<8}  INFO]")), "{line}");
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
    let messages = |text: &str| text.lines().map(|line| line.split_once(' ').unwrap().1.to_string()).collect::<Vec<_>>();
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

#[test]
fn replay_uses_recorded_time_in_local_timezone() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("timed.jsonl");
    let entries = [
        json!({"version": 1, "started_at": "2000-01-01T03:04:05.990Z"}),
        json!({"kind": "event", "elapsed_us": 0, "envelope": {
            "scope": {}, "event": {"Log": {"level": "INFO", "message": "recorded event"}}
        }}),
        json!({"kind": "output", "elapsed_us": 20000, "command": 1,
            "stream": "stderr", "lix": true,
            "bytes": b"@nix {\"action\":\"msg\",\"level\":3,\"msg\":\"parsed output\"}\n".to_vec()}),
        json!({"kind": "output", "elapsed_us": 20000, "command": 2,
            "stream": "stdout", "lix": false, "bytes": b"partial line".to_vec()}),
        json!({"kind": "exit", "elapsed_us": 20000, "command": 2, "success": true}),
    ];
    std::fs::write(&path, entries.iter().map(|entry| format!("{entry}\n")).collect::<String>()).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_wat"))
        .arg("replay").arg(path)
        .env("TZ", "XXX-2")
        .env("RUST_LOG", "info").env("RUST_LOG_STYLE", "never")
        .output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stderr = String::from_utf8(output.stderr).unwrap();
    let lines: Vec<_> = stderr.lines().collect();
    assert_eq!(lines.len(), 3, "{stderr}");
    for (line, time, message) in [
        (lines[0], "05:04:05", "recorded event"),
        (lines[1], "05:04:06", "parsed output"),
        (lines[2], "05:04:06", "partial line"),
    ] {
        assert!(line.starts_with(&format!("[{time} ")), "{line}");
        assert!(line.ends_with(message), "{line}");
    }
}

#[test]
fn replay_reclassifies_legacy_service_errors_but_preserves_other_host_failures() {
    use serde_json::json;
    for other_failure in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("activation.jsonl");
        let mut entries = vec![json!({"version": 1})];
        let event = |event| json!({"kind": "event", "elapsed_us": 0,
            "envelope": {"scope": {}, "event": event}});
        entries.push(event(json!({"Pipeline": {"Planned": {"stages": [
            ["sirrah", ["Build", "Deploy"]], ["other", ["Build"]]
        ]}}})));
        for (host, phase, success) in [("sirrah", "Build", true), ("other", "Build", !other_failure)] {
            entries.push(event(json!({"Host": {"Finished": {"host": host, "phase": phase, "success": success}}})));
        }
        entries.push(event(json!({"Host": {"Started": {"host": "sirrah", "phase": "Activate"}}})));
        entries.push(json!({"kind": "command", "elapsed_us": 0, "command": 1,
            "program": "ssh", "args": ["root@sirrah", "/nix/store/system/bin/switch-to-configuration test && sync"]}));
        entries.push(json!({"kind": "exit", "elapsed_us": 0, "command": 1, "code": 4, "success": false}));
        for phase in ["Activate", "Deploy"] {
            entries.push(event(json!({"Host": {"Finished": {"host": "sirrah", "phase": phase, "success": false}}})));
        }
        entries.push(event(json!({"Pipeline": {"Finished": {"success": false}}})));
        let mut error = "failed to switch configuration remotely on sirrah: ssh exited with status exit status: 4".to_owned();
        if other_failure { error.push_str("\nfailed to build other: lix exited with status exit status: 1"); }
        entries.push(json!({"kind": "outcome", "elapsed_us": 0, "error": error}));
        std::fs::write(&path, entries.iter().map(|entry| format!("{entry}\n")).collect::<String>()).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_wat"))
            .arg("replay").arg(&path).env("RUST_LOG", "info").output().unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output.status.success(), !other_failure, "{stderr}");
        assert!(stderr.contains("sirrah") && stderr.contains("activation reported service errors"), "{stderr}");
        assert!(!stderr.contains("failed to activate"), "{stderr}");
        assert!(!stderr.contains("exited with status"), "{stderr}");
        if other_failure {
            assert!(stderr.contains("1 machine failed to build: other"), "{stderr}");
            assert!(stderr.ends_with("1 / 0 / 0 / 1  # failed/build/deploy/done\n"), "{stderr}");
        } else {
            assert!(stderr.ends_with("0 / 0 / 0 / 2  # failed/build/deploy/done\n"), "{stderr}");
        }
    }
}

#[test]
fn replay_demotes_hostkeys_from_both_lix_and_direct_ssh_output() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hostkeys.jsonl");
    let hostkey = "Host key fingerprint is: SHA256:example";
    let art = ["+--[ED25519 256]--+", "|  E.. .   o.+o   |", "+----[SHA256]-----+"];
    let mut entries = vec![json!({"version": 1})];
    for (command, lix) in [(1, true), (2, false)] {
        for line in std::iter::once(hostkey).chain(art).chain(["Checking switch inhibitors... done"]) {
            entries.push(json!({"kind": "output", "elapsed_us": 0, "command": command,
                "scope": {"host": "example", "phase": "Deploy"},
                "stream": "stderr", "lix": lix, "bytes": format!("{line}\n").as_bytes()}));
        }
    }
    std::fs::write(&path, entries.iter().map(|entry| format!("{entry}\n")).collect::<String>()).unwrap();
    for level in ["info", "debug"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_wat"))
            .arg("replay").arg(&path).env("RUST_LOG", level).env("RUST_LOG_STYLE", "never")
            .output().unwrap();
        assert!(output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(stderr.matches("Checking switch inhibitors... done").count(), 2, "{stderr}");
        for line in std::iter::once(hostkey).chain(art) {
            assert_eq!(stderr.matches(line).count(), if level == "debug" { 2 } else { 0 }, "{stderr}");
        }
        for line in stderr.lines().filter(|line| line.contains(hostkey) || art.iter().any(|art| line.ends_with(art))) {
            assert!(line.contains("DEBUG]"), "{line}");
        }
    }
}

#[test]
fn replay_demotes_multiline_derivation_plans_without_hiding_diagnostics() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("derivations.jsonl");
    let message = "these \x1b[35;1m13\x1b[0m derivations will be built:\n  \x1b[35;1m/nix/store/example-system-path.drv\x1b[0m\nordinary diagnostic";
    let raw = format!("@nix {}\n", json!({"action": "msg", "level": 3, "msg": message}));
    let entries = [json!({"version": 1}), json!({"kind": "output", "elapsed_us": 0,
        "command": 1, "stream": "stderr", "lix": true, "bytes": raw.as_bytes()})];
    std::fs::write(&path, entries.iter().map(|entry| format!("{entry}\n")).collect::<String>()).unwrap();
    for level in ["info", "debug"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_wat"))
            .arg("replay").arg(&path).env("RUST_LOG", level).output().unwrap();
        assert!(output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("ordinary diagnostic"), "{stderr}");
        for detail in ["derivations will be built:", "/nix/store/example-system-path.drv"] {
            assert_eq!(stderr.contains(detail), level == "debug", "{stderr}");
        }
    }
}

#[test]
fn replay_accepts_truncated_tail_but_rejects_corrupt_or_nonfinal_entries() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("incomplete.jsonl");
    let output_entry = json!({"kind": "output", "elapsed_us": 0, "command": 1,
        "stream": "stdout", "lix": false, "bytes": b"last complete output".to_vec()});
    let prefix = format!("{{\"version\":1}}\n{output_entry}\n");
    for (tail, success) in [
        ("{\"bytes\":[1,2,", true),
        ("{\"kind\":\"out", true),
        ("{\"bytes\":[1,2,\n{}\n", false),
        ("{garbage}", false),
    ] {
        std::fs::write(&path, format!("{prefix}{tail}")).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_wat"))
            .arg("replay").arg(&path).env("RUST_LOG", "info").output().unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output.status.success(), success, "{stderr}");
        if success {
            assert!(stderr.contains("last complete output"), "{stderr}");
            assert!(stderr.contains("Ignoring incomplete final recording entry"), "{stderr}");
        }
    }
}
