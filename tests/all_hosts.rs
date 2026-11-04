#[cfg(unix)]
#[test]
fn all_discovers_hosts_before_building() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};

    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("flake.nix"), "{}").unwrap();
    let nix = dir.path().join("fake-nix");
    fs::write(
        &nix,
        "#!/bin/sh\nif [ \"$1\" = eval ]; then\n  printf '[\"alpha\",\"beta\"]'\n  exit 0\nfi\nprintf '%s\\n' \"$*\" >> builds\nexit 7\n",
    )
    .unwrap();
    fs::set_permissions(&nix, fs::Permissions::from_mode(0o700)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_wat"))
        .current_dir(dir.path())
        .env("WAT_NIX_BINARY", &nix)
        .args(["build", "--all"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let builds = fs::read_to_string(dir.path().join("builds")).unwrap();
    assert!(builds.contains("nixosConfigurations.alpha."), "{builds}");
    assert!(builds.contains("nixosConfigurations.beta."), "{builds}");
}
