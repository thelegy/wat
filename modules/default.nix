inputs@{ ... }:
let

  flakeModules.default = import ./wat inputs;

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
