flakes@{ self, ... }:

rec {

  withPkgsFor = systems: nixpkgs: overlays: fn: with nixpkgs.lib;
    genAttrs systems (system: fn (import nixpkgs { inherit system overlays; }));

  withPkgsForLinux = nixpkgs: withPkgsFor nixpkgs.lib.platforms.linux nixpkgs;

  baseFlake = import ./baseFlake.nix flakes;
  mkModule = import ./mkModule.nix flakes;
  mkMachine = import ./mkMachine.nix flakes;
  mkWatRepo = import ./mkWatRepo.nix flakes;

  dependencyDagOfSubmodule = import ./dependencyDagOfSubmodule.nix;

}
