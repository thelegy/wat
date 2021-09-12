{ writeShellScript, nixFlakes }:

writeShellScript "nix" ''

  ${nixFlakes}/bin/nix --log-format bar-with-logs  --experimental-features "nix-command flakes" "$@"
''
