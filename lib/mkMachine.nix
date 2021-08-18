{ self, ... }:
with self.lib;


{ flakes ? {}, extraModules ? [] }:

name:

{ nixpkgs ? flakes.nixpkgs
, system ? "x86_64-linux"
, loadModules ? []
}:

module:

nixosSystem {
  inherit system;
  modules = [({ config, lib, ... }: {
    imports = attrValues extraModules ++ loadModules ++ [ module ];

    nixpkgs.overlays = [ ];

    _module.args.flakes = flakes;

    nix.nixPath = [ "nixpkgs=${nixpkgs}" ];
    nix.registry.nixpkgs.flake = nixpkgs;
    nix.registry.n.flake = nixpkgs;
    networking.hostName = lib.mkDefault name;
  })];
}
