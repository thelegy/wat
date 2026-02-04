inputs@{ ... }:
{

  flakeModules.default = import ./flakeModule.nix inputs;

  lib = import ../nix/lib inputs;

  nixosModules = import ../nix/modules inputs;

  overlays.default = import ../nix/overlay inputs;

}
