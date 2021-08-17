{ self, nixpkgs, ... }:
with self.lib;


name:
system:
module:

nixosSystem {
  inherit system;
  modules = [({ config, lib, ... }: {
    imports = [ module ];

    nixpkgs.overlays = [ ];

    nix.nixPath = [ "nixpkgs=${nixpkgs}" ];
    nix.registry.nixpkgs.flake = nixpkgs;
    nix.registry.n.flake = nixpkgs;
    networking.hostName = lib.mkDefault name;
  })];
}
