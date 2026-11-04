# Wat

## Selecting hosts

Pass one or more hostnames, or use `-A` / `--all` to target every host in the
current flake's `nixosConfigurations`:

```sh
wat build my-host other-host
wat build --all
wat switch -A
```

The flag works with all host-targeting commands and cannot be combined with
explicit hostnames. An empty host list is an error.

## Shell Completions

Generate completion scripts by setting the `COMPLETE` environment variable when invoking `wat`, then install the output where your shell expects:

```sh
COMPLETE=bash wat > /etc/bash_completion.d/wat
COMPLETE=zsh wat > "${ZDOTDIR:-$HOME}/.zsh/completions/_wat"
COMPLETE=fish wat > ~/.config/fish/completions/wat.fish
```

Supported shells include `bash`, `elvish`, `fish`, `powershell`, and `zsh`.

### Recording and replaying output

Every normal run saves a versioned JSONL recording under `.wat/logs` in the
current working repository (alongside `flake.nix`). The directory is created
automatically, and `.wat/.gitignore` gets a `/logs/` entry while preserving existing
entries. Each run prints its unique recording path at startup. Recordings persist
until you remove them; there is no automatic cleanup.

```console
wat switch my-host
wat replay .wat/logs/wat-events-EXAMPLE.jsonl
```

Replay works outside a flake directory and does not create another recording or
execute Nix, SSH, activation, or any other external commands. It
preserves recorded timing and feeds raw Nix stderr through the current parser,
reducer, and reporter. Lifecycle events, command arguments, stdout/stderr bytes,
and exit statuses are included, so recordings can also be inspected directly.
SSH and activation output is captured and replayed on its original output stream.
Hostname completion is not recorded.

Recordings can contain sensitive command output and host information. Automatic
recordings are private files; remove recordings when no longer needed.

## Connections

All targets use SSH as `root@<hostname>` by default, including the local machine.
Optionally create `.wat/config.toml` alongside the `flake.nix` in the directory
where you invoke `wat`:

```toml
connection = "ssh"

[hosts.my-local-host]
connection = "sudo"
```

`connection` sets the default; `[hosts.<hostname>]` overrides it for an exact
host name. `ssh` copies the closure and activates over SSH. `sudo` activates on
the machine running `wat`, so use it only for local targets. To retain the old
behavior, add a `sudo` override for your local host and leave the default as SSH.

The config library also supports `.wat/config.json`, `.yaml`, `.yml`, `.ini`,
`.ron`, `.json5`, and `.corn`. Use one configuration file. Missing files and
omitted settings use defaults; malformed files and unknown settings are errors.
