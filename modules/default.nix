inputs@{ ... }:
let

  toplevel = inputs // {
    wat = {
      inherit nixosModules overlays;
    };
  };

  flakeModules.default = import ./wat toplevel;

  nixosModules = import ./_nixosModules toplevel;

  overlays.default = import ./_overlay toplevel;

in
{
  inherit
    flakeModules
    nixosModules
    overlays
    ;
}
