flakes@{ self, nixpkgs, ... }:
with nixpkgs.lib;

nixpkgs.lib // rec {

  defaultSystems = platforms.linux;

  eachSystem = systems: fn: let
    perSystem = acc: system: let
      result = fn system pkgs;
      overlays = result.systemOverlays or [];
      pkgs = import nixpkgs { inherit system overlays; };
      fn2 = x: key: if key == "systemOverlays" then x else x // {
        ${key} = (x.${key} or {}) // { ${system} = result.${key}; };
      };
    in foldl' fn2 acc (attrNames result);
  in foldl' perSystem {} systems;

  eachDefaultSystem = eachSystem defaultSystems;

  mkMachine = import ./mkMachine.nix flakes;
  mkWatRepo = import ./mkWatRepo.nix flakes;

}
