{ lib, flakes, ... }:
with lib;
with types;

let

  machineType = attrsOf fileType;
  fileType = submodule (
    { config, ... }:
    {
      options = {
        file = mkOption {
          type = path;
        };
        content = mkOption {
          type = str;
          default = builtins.readFile config.file;
        };
      };
    }
  );

  machineFiles = pipe (flakes.self.nixosConfigurations or { }) [
    (mapAttrs (_key: attrByPath [ "watExtraOutput" "machineFiles" ] { }))
    (mapAttrs (_key1: mapAttrs (_key2: val: { file = val; })))
  ];

in
{

  options = {

    wat.machines = mkOption {
      default = { };
      type = attrsOf machineType;
    };

  };

  config = {
    wat.machines = mkDefault machineFiles;
  };

}
