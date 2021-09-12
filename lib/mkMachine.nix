{ self, ... }:
with self.lib;


{ flakes ? {}
, extraOverlays ? []
, extraModules ? []
}:

name:

{ nixpkgs ? flakes.nixpkgs
, system ? "x86_64-linux"
, loadModules ? []
}:

module:

let

  availableModules = extraModules ++ loadModules;

  baseConfiguration = { config, lib, ... }: {
    nixpkgs.overlays = extraOverlays;

    _module.args = {
      inherit flakes availableModules;
    };

    nix.nixPath = [ "nixpkgs=${nixpkgs}" ];
    nix.registry.nixpkgs.flake = nixpkgs;
    nix.registry.n.flake = nixpkgs;
    networking.hostName = mkDefault name;
  };

in nixosSystem {
  inherit system;
  modules = availableModules ++ [ baseConfiguration module ];
}
