inputs@{ ... }:
let

  lib.mkWatRepo = import ./mkWatRepo.nix inputs;

  flakeModule = ./flakeModule.nix;

  flakeModules.default = flakeModule;

  modules.flake = flakeModules;

  nixosModules = import ./_nixosModules inputs;

  modules.nixos = nixosModules;

  overlays.default = import ./_overlay inputs;

in
{
  inherit
    lib
    flakeModule
    flakeModules
    nixosModules
    overlays
    modules
    ;
  inherit (inputs.dev)
    checks
    tests
    devShells
    formatter
    packages
    ;
}
