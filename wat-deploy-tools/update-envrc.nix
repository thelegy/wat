{ lib, writeScriptBin }:
with lib;

writeScriptBin "update-envrc" ''
  #!/bin/sh

  nix build .#wat-deploy-tools-envrc --out-link .envrc
''
