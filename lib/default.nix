flakes@{ self, ... }:

let

  needsLib = {

    types.dependencyDagOfSubmodule = import ./dependencyDagOfSubmodule.nix;

    mkModule = import ./mkModule.nix;

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
    applyRecursively = x: lib.mapAttrs (k: v: if lib.isFunction v then v x else applyRecursively x v);
    watLib = lib.foldl' (x: y: lib.recursiveUpdate x y) {} [
      lib
      (applyRecursively watLib needsLib)
      selfContained
    ];
  in watLib;

in selfContained // {
  inherit bake;
}
