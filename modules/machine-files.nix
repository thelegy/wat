{ lib, ... }:
with lib;
with types;

let

  machineType = attrsOf fileType;
  fileType = submodule {
    options = {
      file = mkOption {
        type = path;
      };
      content = mkOption {
        type = str;
      };
    };
  };

in {

  options = {

    wat.machines = mkOption {
      default = {};
      type = attrsOf machineType;
    };

    wat.build.machineFiles = mkOption {
      type = machineType;
      default = {};
    };

  };

}
