#![cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt, process::Command};
use serde_json::Value;

#[test]
fn reports_preserve_stdout_and_capture_mixed_outcomes() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("flake.nix"), "{}").unwrap();
    let store = dir.path().join("store");
    fs::create_dir(&store).unwrap();
    let nix = dir.path().join("nix");
    fs::write(&nix, r#"#!/bin/sh
if [ "$1" = eval ]; then printf '["good","bad"]'; exit 0; fi
if [ "$3" = eval ]; then
  case "$*" in *nixosConfigurations.bad.*) printf /nix/store/bad.drv;; *) printf /nix/store/good.drv;; esac
  exit 0
fi
case "$*" in */bad.drv*) echo 'evaluation or build failed' >&2; exit 7;; esac
while [ "$#" -gt 0 ] && [ "$1" != --out-link ]; do shift; done
ln -s "$STORE" "$2"
"#).unwrap();
    fs::set_permissions(&nix, fs::Permissions::from_mode(0o700)).unwrap();
    for hosts in [vec!["good"], vec!["good", "bad"], vec!["--all"], vec!["-A"]] {
        let run = |report: bool| {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_wat"));
            cmd.current_dir(dir.path()).env("WAT_NIX_BINARY", &nix).env("STORE", &store)
                .arg("build").args(&hosts);
            if report { cmd.args(["--results-file", "results.json"]); }
            cmd.output().unwrap()
        };
        let normal = run(false);
        let reported = run(true);
        assert_eq!(normal.stdout, reported.stdout);
        assert_eq!(normal.status.code(), reported.status.code());
        let data: Value = serde_json::from_slice(&fs::read(dir.path().join("results.json")).unwrap()).unwrap();
        assert_eq!(data["version"], 1);
        assert_eq!(data["status"], "complete");
        assert_eq!(data["hosts"]["good"]["status"], "success");
        assert_eq!(reported.status.success(), hosts == ["good"]);
        if !reported.status.success() {
            assert_eq!(data["hosts"]["bad"]["status"], "build-failed");
            assert!(data["hosts"]["bad"]["reason"].as_str().unwrap().contains("failed to build"));
        }
    }
    // Replace a previous successful report even if setup fails before discovery.
    fs::remove_file(dir.path().join("flake.nix")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_wat")).current_dir(dir.path())
        .args(["build", "--all", "--results-file", "results.json"]).output().unwrap();
    assert!(!output.status.success());
    let data: Value = serde_json::from_slice(&fs::read(dir.path().join("results.json")).unwrap()).unwrap();
    assert_eq!(data["status"], "incomplete");
    assert!(data["reason"].as_str().unwrap().contains("flake.nix"));
    let output = Command::new(env!("CARGO_BIN_EXE_wat")).current_dir(dir.path())
        .args(["build", "good", "--results-file", "missing/results.json"]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to write results file"));
}
