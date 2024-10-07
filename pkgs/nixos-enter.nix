{ lib
, writeShellScriptBin
, coreutils
, util-linux
, systemd
, nixpkgs
, system
}: with lib;

let

  configuration = import "${nixpkgs}/nixos/lib/eval-config.nix" {
    modules = [];
    system = system;
  };

in writeShellScriptBin "nixos-enter" ''
  export PATH=${coreutils}/bin:${util-linux}/bin:${systemd}/bin:$PATH
  ${configuration.pkgs.nixos-enter}/bin/nixos-enter "$@"
''
