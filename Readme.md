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

### Machine-readable build results

The legacy Python prebuild tool has been removed. Replace `nix run .#prebuild-script` (or `wat-prebuild`) in consuming flakes with:

```sh
nix develop -c wat build --all --results-file ./build-results.json
```

This is a breaking change: the `packages.<system>.prebuild-script` output and
`wat-prebuild` / `wat-prebuild-script` overlay packages are no longer provided.
Remove `wat.enableAutoBuildTargets` and `wat.extraBuildTargets` from flake
configuration; these options have also been removed. `--all` selects NixOS hosts;
use explicit hostnames for a subset. Arbitrary extra Nix build targets must be
built separately with `nix build`.

Update automation to parse the JSON report described below instead of the old
numeric, line-based output. Compare per-host outcomes and validate the report's
version and completeness, including on command failure. The legacy evaluation
and build timeouts and separate evaluation-failure categories are not retained;
configure any required timeout in the calling job. The wat CLI remains in the
dev shell, and build progress and human interaction are unchanged.

`wat build [HOST ... | --all | -A] --results-file PATH` writes a versioned
JSON report without changing terminal rendering, logs, or stdout store paths.
The option is available only for `build`. For example:

```json
{
  "version": 1,
  "status": "complete",
  "hosts": {
    "sirrah": { "status": "success" },
    "y": { "status": "build-failed", "reason": "failed to build ..." }
  }
}
```

Version 1 has a top-level `status` of `complete` or `incomplete` and a `hosts`
object keyed by hostname, serialized in sorted order. `complete` means every
selected host reached an outcome, not that every host succeeded. Host statuses
are `success`, `build-failed`, or `incomplete`. `build-failed` includes evaluation,
Nix invocation, and output resolution failures; its `reason` contains the error
chain for human diagnosis and is not a stable machine-readable category.
An incomplete run includes a top-level `reason` when available. Discovery or
setup failures can leave `hosts` empty. Consumers must check version, run status,
and host outcomes; comparison and baseline policy belong to the consumer.

Before setup begins, wat replaces the report with an `incomplete` report, so an
interrupted run cannot leave a previous successful report in place. On normal
termination it replaces the report again, including when hosts fail. Unfinished
hosts remain `incomplete`. Abrupt termination may leave only the initial report;
parse errors and runtime initialization failures may prevent any report. Consumers
must reject missing or malformed reports and ensure they read the current run's
file. No timeout or interruption handler is added.

Writes use a synced temporary file in the destination directory followed by atomic
replacement. Parent directories must already exist. Report write failures are
printed to stderr and cause failure exit status. Individual host failures retain
the existing aggregate failure exit status. Reports contain no timestamps or
completion-order-dependent fields; error text can include environment-specific
paths. Report paths should be dedicated output files, separate from inputs and logs.
