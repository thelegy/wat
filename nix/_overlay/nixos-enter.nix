{
  writeShellScriptBin,
  coreutils,
  util-linux,
  systemd,
  path,
  system,
  nixos-enter ? null,
}:

let

  configuration = import "${path}/nixos/lib/eval-config.nix" {
    modules = [ ];
    system = system;
  };

  unwrapped-enter = if isNull nixos-enter then configuration.pkgs.nixos-enter else nixos-enter;

in
writeShellScriptBin "nixos-enter" ''
  export PATH=${coreutils}/bin:${util-linux}/bin:${systemd}/bin:$PATH
  ${unwrapped-enter}/bin/nixos-enter "$@"
''
