inputs@{ ... }:
let

  toplevel = inputs // {
    wat = {
      inherit lib nixosModules overlays;
    };
  };

  lib = import ./_lib;

  flakeModules.default = import ./wat toplevel;

  nixosModules = import ../nix/modules toplevel;

  overlays.default = import ../nix/overlay toplevel;

in
{
  inherit
    flakeModules
    nixosModules
    overlays
    ;
}
