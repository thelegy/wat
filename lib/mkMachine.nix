{ self, ... }:
with self.lib;


{ flakes ? {}, extraModules ? [] }:
name:
nixpkgs:
system:
module:

nixosSystem {
  inherit system;
  modules = [({ config, lib, ... }: {
    imports = attrValues extraModules ++ [ module ];

    nixpkgs.overlays = [ ];

    _module.args.flakes = flakes;

    nix.nixPath = [ "nixpkgs=${nixpkgs}" ];
    nix.registry.nixpkgs.flake = nixpkgs;
    nix.registry.n.flake = nixpkgs;
    networking.hostName = lib.mkDefault name;
  })];
}
