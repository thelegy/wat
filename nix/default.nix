inputs@{ ... }:
let

  flakeModules.default = ./flakeModule.nix;

  modules.flake = flakeModules;

  nixosModules = import ./_nixosModules inputs;

  modules.nixos = nixosModules;

  overlays.default = import ./_overlay inputs;

in
{
  flakeModule = flakeModules.default;
  inherit
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
