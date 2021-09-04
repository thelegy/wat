{ self, ... }:
with self.lib;


flakes@{ nixpkgs, ... }:
fn:

let

  dontLoadFlakeModules = result.dontLoadFlakeModules or false;
  dontLoadFlakeOverlay = result.dontLoadFlakeOverlay or false;
  dontLoadWatModules = result.dontLoadWatModules or false;

  extraOverlays = (result.loadOverlays or [])
    ++ (optionals (!dontLoadFlakeOverlay) (toList (flakes.self.overlay or [])))
  ;

  extraModules = (result.loadModules or [])
    ++ (optionals (!dontLoadWatModules) (attrValues self.nixosModules))
    ++ (optionals (!dontLoadFlakeModules) (attrValues (flakes.self.nixosModules or {})))
  ;

  machineArgs = name: {
    inherit flakes;
    mkMachine = mkMachine { inherit flakes extraOverlays extraModules; } name;
  };

  args = {

    findModules = namespace: dir: let
      moduleNames = pipe dir [
        builtins.readDir
        (filterAttrs (key: val: ! hasPrefix "." key && (hasSuffix ".nix" key || val == "directory")))
        attrNames
      ];
    in listToAttrs (forEach moduleNames (name: mkModule {
      path = dir + "/${name}";
      namespace = [ "wat" ] ++ namespace;
    }));

    findMachines = dir: let
      machineNames = pipe dir [
        builtins.readDir
        (filterAttrs (key: val: ! hasPrefix "." key && val == "directory"))
        attrNames
      ];
    in genAttrs machineNames (name: import (dir + "/${name}") (machineArgs name));

  };

  result = fn args;

in recursiveUpdate baseFlake result.outputs
