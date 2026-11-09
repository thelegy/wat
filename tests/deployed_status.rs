#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, process::Command, time::{Duration, Instant}};

#[test]
fn deployment_probe_uses_configured_connection_and_never_delays_build() {
    for connection in ["ssh", "sudo"] {
        for probe in ["update", "failed", "slow"] {
            let dir = tempfile::tempdir().unwrap();
            fs::create_dir(dir.path().join(".wat")).unwrap();
            fs::write(dir.path().join(".wat/config.toml"), format!("connection = '{connection}'")).unwrap();
            fs::write(dir.path().join("flake.nix"), "{}").unwrap();
            fs::create_dir(dir.path().join("store")).unwrap();
            for (name, script) in [
                ("nix", r#"#!/bin/sh
case "$3" in
 eval) sleep 0.2; test -f probe-started || exit 9; printf /nix/store/example.drv ;;
 build)
 while [ "$1" != --out-link ]; do shift; done
 ln -s "$PWD/store" "$2" ;;
esac
"#),
                (connection, r#"#!/bin/sh
printf '%s\n' "$*" > probe-started
case "$PROBE" in
 update) printf /nix/store/old-system ;;
 failed) exit 1 ;;
 slow) exec sleep 30 ;;
esac
"#),
            ] {
                let path = dir.path().join(name);
                fs::write(&path, script).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let mut child = Command::new(env!("CARGO_BIN_EXE_wat"))
                .current_dir(dir.path())
                .env("WAT_NIX_BINARY", dir.path().join("nix"))
                .env("PATH", format!("{}:{}", dir.path().display(), std::env::var("PATH").unwrap()))
                .env("PROBE", probe)
                .args(["build", "example"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn().unwrap();
            let start = Instant::now();
            while child.try_wait().unwrap().is_none() {
                if start.elapsed() > Duration::from_secs(5) {
                    child.kill().unwrap();
                    panic!("build waited for {connection} probe ({probe})");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let output = child.wait_with_output().unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(output.status.success(), "{stderr}");
            let icon = if probe == "update" { "↑" } else { "?" };
            assert!(stderr.lines().any(|line| line.contains("Built:") && line.ends_with(icon)), "{stderr}");
            let args = fs::read_to_string(dir.path().join("probe-started")).unwrap();
            assert!(args.contains("readlink -e /run/current-system"));
            assert_eq!(args.contains("root@example"), connection == "ssh");
        }
    }
}
