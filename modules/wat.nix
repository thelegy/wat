{ inputs, ... }:
{

  systems = [
    "x86_64-linux"
    "aarch64-linux"
  ];

  imports = [
    inputs.flake-aspects.flakeModule
  ];

  flake = {

    lib = import ../nix/lib inputs;

    nixosModules = import ../nix/modules inputs;

    overlays.default = import ../nix/overlay inputs;

  };

}
