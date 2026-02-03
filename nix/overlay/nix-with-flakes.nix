{ writeShellScript, nixVersions }:

writeShellScript "nix" ''

  ${nixVersions.stable}/bin/nix --log-format bar-with-logs --experimental-features "nix-command flakes" "$@"
''
