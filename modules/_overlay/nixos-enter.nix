{ lib
, writeShellScriptBin
, coreutils
, util-linux
, systemd
, nixpkgs
, system
, nixos-enter ? null
}: with lib;

let

  configuration = import "${nixpkgs}/nixos/lib/eval-config.nix" {
    modules = [];
    system = system;
  };

  unwrapped-enter = if isNull nixos-enter then configuration.pkgs.nixos-enter else nixos-enter;

in writeShellScriptBin "nixos-enter" ''
  export PATH=${coreutils}/bin:${util-linux}/bin:${systemd}/bin:$PATH
  ${unwrapped-enter}/bin/nixos-enter "$@"
''
