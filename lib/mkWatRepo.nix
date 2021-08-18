{ self, ... }:
with self.lib;


flakes@{ nixpkgs, ... }:
fn:

let

  args = {

    findMachines = dir: let
      machineNames = pipe dir [
        builtins.readDir
        (filterAttrs (key: val: ! hasPrefix "." key && val == "directory"))
        attrNames
      ];
    in genAttrs machineNames (name: import (traceVal (dir +"/${name}")) (machineArgs name));

   };

   machineArgs = name: {
    inherit flakes;
    mkMachine = mkMachine flakes name;
   };

   result = fn args;

in baseFlake // result.outputs
