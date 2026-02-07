inputs@{ ... }:
let

  flakeModules.default = import ./flakeModule.nix inputs;

  nixosModules = import ./_nixosModules inputs;

  overlays.default = import ./_overlay inputs;

in
{
  inherit
    flakeModules
    nixosModules
    overlays
    ;
}
