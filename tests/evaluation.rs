#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, process::Command};
use serde_json::Value;

#[test]
fn evaluation_precedes_derivation_build_and_scopes_diagnostics() {
    for fail_eval in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("flake.nix"), "{}").unwrap();
        let store = dir.path().join("store");
        fs::create_dir(&store).unwrap();
        let nix = dir.path().join("nix");
        fs::write(&nix, r#"#!/bin/sh
printf '%s\n' "$*" >> commands
case "$3" in
  eval)
    printf '@nix {"action":"msg","level":1,"msg":"evaluation warning"}\n' >&2
    [ "$FAIL_EVAL" = true ] && exit 7
    printf /nix/store/example.drv
    ;;
  build)
    printf '@nix {"action":"msg","level":1,"msg":"build warning"}\n' >&2
    while [ "$#" -gt 0 ] && [ "$1" != --out-link ]; do shift; done
    ln -s "$STORE" "$2"
    ;;
esac
"#).unwrap();
        fs::set_permissions(&nix, fs::Permissions::from_mode(0o700)).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_wat"))
            .current_dir(dir.path()).env("WAT_NIX_BINARY", &nix)
            .env("STORE", &store).env("FAIL_EVAL", fail_eval.to_string())
            .args(["build", "example"]).output().unwrap();
        assert_eq!(output.status.success(), !fail_eval, "{}", String::from_utf8_lossy(&output.stderr));
        let commands = fs::read_to_string(dir.path().join("commands")).unwrap();
        let commands: Vec<_> = commands.lines().collect();
        assert!(commands[0].contains("eval --raw .#nixosConfigurations.example.config.system.build.toplevel.drvPath"));
        assert_eq!(commands.len(), if fail_eval { 1 } else { 2 });
        if !fail_eval {
            assert!(commands[1].contains("build --keep-going /nix/store/example.drv^* --out-link"));
        }
        let recording = fs::read_dir(dir.path().join(".wat/logs")).unwrap().next().unwrap().unwrap().path();
        let entries: Vec<Value> = fs::read_to_string(recording).unwrap().lines()
            .map(|line| serde_json::from_str(line).unwrap()).collect();
        for (message, phase) in [("evaluation warning", "Evaluate"), ("build warning", "Build")] {
            if fail_eval && phase == "Build" { continue; }
            assert!(entries.iter().any(|entry| entry["kind"] == "output"
                && entry["scope"]["phase"] == phase
                && entry["bytes"].as_array().is_some_and(|bytes| {
                    let bytes: Vec<u8> = bytes.iter().map(|byte| byte.as_u64().unwrap() as u8).collect();
                    String::from_utf8_lossy(&bytes).contains(message)
                })));
        }
        if fail_eval {
            assert!(entries.iter().any(|entry| entry["envelope"]["event"]["Host"]["Finished"]["phase"] == "Evaluate"
                && entry["envelope"]["event"]["Host"]["Finished"]["success"] == false));
        }
    }
}
