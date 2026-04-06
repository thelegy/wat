# Wat

## Shell Completions

Generate completion scripts with the `completion` subcommand and install them where your shell expects:

```sh
wat completion bash > /etc/bash_completion.d/wat
wat completion zsh > "${ZDOTDIR:-$HOME}/.zsh/completions/_wat"
wat completion fish > ~/.config/fish/completions/wat.fish
```

Supported shells include `bash`, `elvish`, `fish`, `powershell`, and `zsh`.
