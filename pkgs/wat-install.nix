{ writeScriptBin
, nix-with-flakes
, zsh
, git
, lib
}:
with lib;

writeScriptBin "wat-install" ''
#!${zsh}/bin/zsh
  set -euo pipefail

  # reset path to ensure the independence of this script
  path=(${escapeShellArgs [
    "${git}/bin"
  ]})

  export FLAKE
  : ''${FLAKE:=.}

  export machine=$1
  shift

  export system=$(${nix-with-flakes} eval --impure --raw --expr 'builtins.currentSystem')

  exec ${nix-with-flakes} run $FLAKE#nixosConfigurations.$machine.config.wat.build.installer.launcher.script.$system -- "$@"
''
