{ inputs, ... }:
{

  systems = [
    "x86_64-linux"
    "aarch64-linux"
  ];

  flake = {

    lib = import ../nix/lib inputs;

    nixosModules = import ../nix/modules inputs;

    overlays.default = import ../nix/overlay inputs;

  };

}
