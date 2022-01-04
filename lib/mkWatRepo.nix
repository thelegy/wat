{ self, ... }:
with self.lib;


flakes@{ nixpkgs, ... }:
fn:

let

  dontLoadFlakeModules = result.dontLoadFlakeModules or false;
  dontLoadFlakeOverlay = result.dontLoadFlakeOverlay or false;
  dontLoadWatModules = result.dontLoadWatModules or false;
  dontLoadWatOverlay = result.dontLoadWatOverlay or false;
  namespacePrefix = result.namespacePrefix or [ "wat" ];
  repoUuidModule = { wat-installer-lib, ... }: {
    wat.installer.repoUuid = foldl'
      (namespace: name: wat-installer-lib.uuidgen { inherit namespace name;})
      "59d93334-df87-4242-ac91-9c48886b4d94"
      (result.namespace or []);
  };

  extraOverlays = (result.loadOverlays or [])
    ++ (optionals (!dontLoadFlakeOverlay) (toList (flakes.self.overlay or [])))
    ++ (optionals (!dontLoadWatOverlay) (toList (self.overlay or [])))
  ;

  extraModules = (result.loadModules or [])
    ++ (optionals (!dontLoadWatModules) (attrValues self.nixosModules))
    ++ (optionals (!dontLoadWatModules) [repoUuidModule])
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
      namespace = namespacePrefix ++ namespace;
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
