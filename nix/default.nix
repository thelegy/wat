inputs@{ ... }:
let

  flakeModule = ./flakeModule.nix;

  flakeModules.default = flakeModule;

  modules.flake = flakeModules;

  nixosModules = import ./_nixosModules inputs;

  modules.nixos = nixosModules;

  overlays.default = import ./_overlay inputs;

in
{
  inherit
    flakeModule
    flakeModules
    nixosModules
    overlays
    modules
    ;
  inherit (inputs.dev)
    checks
    devShells
    formatter
    packages
    ;
}
