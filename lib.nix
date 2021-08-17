{ self, nixpkgs, ... }:
with nixpkgs.lib;

let
  util = import ./helpers.nix {};
in nixpkgs.lib // rec {

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

  mkMachine = name: system: module: let

  in nixosSystem {
    inherit system;
    modules = [({ config, lib, ... }: {
      imports = [ module ];

      nixpkgs.overlays = [ ];

      system.configurationRevision = lib.mkIf (flakes.self ? rev) flakes.self.rev;
      nix.nixPath = [ "nixpkgs=${pkgs}" ];
      nix.registry.nixpkgs.flake = pkgs;
      nix.registry.n.flake = pkgs;

      networking.hostName = lib.mkDefault name;
    })];
  };

  mkWatRepo = fn: let
    args = {
      findMachines = dir: let

        machineNames = with util; (readFilterDir (filterAnd [(not filterDirHidden) filterDirDirs]) dir);
      in genAttrs machineNames (name: import (traceVal (dir +"/${name}")) (machineArgs name));
    };
    machineArgs = name: {
      mkMachine = mkMachine name;
    };
    result = fn args;
  in self.baseFlake // result;

}
