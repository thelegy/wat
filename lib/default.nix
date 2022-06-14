flakes@{ self, ... }:

let

  needsLib = lib: {

    mkModule = import ./mkModule.nix lib;

  };

  selfContained = rec {

    withPkgsFor = systems: nixpkgs: overlays: fn: with nixpkgs.lib;
      genAttrs systems (system: fn (import nixpkgs { inherit system overlays; }));

    withPkgsForLinux = nixpkgs: withPkgsFor nixpkgs.lib.platforms.linux nixpkgs;

    baseFlake = import ./baseFlake.nix flakes;
    mkMachine = import ./mkMachine.nix flakes;
    mkWatRepo = import ./mkWatRepo.nix flakes;

  };

  bake = lib:
  let
    watLib = lib.foldl' (x: y: lib.recursiveUpdate x y) {} [
      (flakes.dependencyDagOfSubmodule.lib.bake lib)
      (needsLib watLib)
      selfContained
    ];
  in watLib;

in selfContained // {
  inherit bake;
}
