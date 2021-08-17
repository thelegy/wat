{ self, ... }:
with self.lib;

fn: let

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
    mkMachine = mkMachine name;
   };

   result = fn args;

in self.baseFlake // result.outputs
