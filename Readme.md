# Wat

## Shell Completions

Generate completion scripts by setting the `COMPLETE` environment variable when invoking `wat`, then install the output where your shell expects:

```sh
COMPLETE=bash wat > /etc/bash_completion.d/wat
COMPLETE=zsh wat > "${ZDOTDIR:-$HOME}/.zsh/completions/_wat"
COMPLETE=fish wat > ~/.config/fish/completions/wat.fish
```

Supported shells include `bash`, `elvish`, `fish`, `powershell`, and `zsh`.
