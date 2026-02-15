{
  nixpkgs,
  system,
}:

let

  configuration = import "${nixpkgs}/nixos/lib/eval-config.nix" {
    modules = [ ];
    system = system;
  };

in
configuration.config.system.build.nixos-generate-config
