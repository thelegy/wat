{ self, ... }:
with self.lib;


flakes@{ nixpkgs, ... }:
fn:

let

  machineArgs = name: {
    inherit flakes;
    mkMachine = mkMachine {inherit flakes; extraModules = result.loadModules or [];} name;
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

in baseFlake // result.outputs
