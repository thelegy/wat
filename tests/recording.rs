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
