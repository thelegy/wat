#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, process::Command};
use serde_json::Value;

#[test]
fn healthy_hosts_finish_despite_build_copy_or_activation_failures() {
    for failure in ["build", "copy", "activate", "service-errors"] {
        for fail_early in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let bin = dir.path().join("bin");
            fs::create_dir(&bin).unwrap();
            let store = dir.path().join("store");
            fs::create_dir(&store).unwrap();
            fs::write(dir.path().join("flake.nix"), "{}").unwrap();
            // Fail the first host deterministically. Serial deployment also exercises
            // stopping queued hosts, without interrupting a running activation.
            let nix = r#"#!/bin/sh
case "$3" in
  build)
    case "$*" in *nixosConfigurations.bad.*) [ "$FAILURE" = build ] && exit 7;; esac
    case "$*" in *nixosConfigurations.healthy.*) sleep 0.1;; esac
    while [ "$#" -gt 0 ] && [ "$1" != --out-link ]; do shift; done
    [ "$#" -ge 2 ] || exit 10
    ln -s "$STORE" "$2"
    ;;
  copy)
    case "$*" in *root@bad*) [ "$FAILURE" = copy ] && exit 8;; esac
    ;;
esac
exit 0
"#;
            let ssh = r#"#!/bin/sh
case "$*" in *root@bad*)
  [ "$FAILURE" = activate ] && exit 9
  case "$*" in *switch-to-configuration*) [ "$FAILURE" = service-errors ] && exit 4;; esac
  ;;
esac
exit 0
"#;
            for (name, contents) in [("nix", nix), ("ssh", ssh)] {
                let path = bin.join(name);
                fs::write(&path, contents).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let mut command = Command::new(env!("CARGO_BIN_EXE_wat"));
            command.current_dir(dir.path())
                .env("WAT_NIX_BINARY", bin.join("nix"))
                .env("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()))
                .env("STORE", &store)
                .env("FAILURE", failure)
                .env("WAT_DEPLOY_CONCURRENCY", "1")
                .args(["dry-activate", "bad", "healthy"]);
            if fail_early { command.arg("--fail-early"); }
            let output = command.output().unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            if failure == "service-errors" {
                assert!(output.status.success(), "{stderr}");
                assert!(stderr.contains("activation reported service errors"));
                assert!(!stderr.contains("machines failed to"));
                continue;
            }
            assert!(!output.status.success());
            let stage = if failure == "copy" { "upload" } else { failure };
            assert!(stderr.contains(&format!("1 machine failed to {stage}: bad")), "{stderr}");
            assert!(!stderr.contains("exited with status"), "{stderr}");
            let recording = fs::read_dir(dir.path().join(".wat/logs")).unwrap().next().unwrap().unwrap().path();
            let entries: Vec<Value> = fs::read_to_string(recording).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
            let healthy_done = entries.iter().any(|entry| {
                let finished = &entry["envelope"]["event"]["Host"]["Finished"];
                finished["host"] == "healthy" && finished["phase"] == "Deploy" && finished["success"] == true
            });
            if !fail_early { assert!(healthy_done, "{failure}: {:?}", output); }
            if fail_early { assert!(!healthy_done, "{failure}"); }
            assert!(entries.iter().any(|entry| entry["envelope"]["event"]["Pipeline"]["Finished"]["success"] == false));
        }
    }
}
