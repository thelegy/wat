{

  inputs.dependencyDagOfSubmodule = {
    url = "github:thelegy/nix-dependencyDagOfSubmodule";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = flakes@{ nixpkgs, ... }: rec {

    lib = import ./lib flakes;

    nixosModules = import ./modules flakes;

    overlays.default = import ./pkgs flakes;

    checks = lib.withPkgsFor [ "x86_64-linux" ] nixpkgs [ overlays.default ] (import ./checks flakes);

  };

}
