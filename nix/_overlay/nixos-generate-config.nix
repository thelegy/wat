{
  pkgs,
  system,
}:

let

  configuration = import "${pkgs}/nixos/lib/eval-config.nix" {
    modules = [ ];
    system = system;
  };

in
configuration.config.system.build.nixos-generate-config
