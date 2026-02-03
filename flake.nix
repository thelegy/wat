{

  inputs.dependencyDagOfSubmodule = {
    url = "github:thelegy/nix-dependencyDagOfSubmodule";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = flakes@{ nixpkgs, ... }: rec {

    lib = import ./nix/lib flakes;

    nixosModules = import ./nix/modules flakes;

    overlays.default = import ./nix/overlay flakes;

    checks = lib.withPkgsFor [ "x86_64-linux" ] nixpkgs [ overlays.default ] (import ./nix/checks flakes);

  };

}
