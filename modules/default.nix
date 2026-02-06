inputs@{ ... }:
let

  toplevel = inputs // {
    wat = {
      inherit lib nixosModules overlays;
    };
  };

  lib = import ../nix/lib;

  flakeModules.default = import ./flakeModule.nix toplevel;

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
