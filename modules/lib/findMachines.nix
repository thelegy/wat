{
  config,
  inputs,
  lib,
  ...
}:
let
  cfg = config.wat;
  flakes = inputs;

  wat.lib.findMachines =
    dir:
    let
      machineNames = lib.pipe dir [
        builtins.readDir
        (lib.filterAttrs (key: val: !lib.hasPrefix "." key && val == "directory"))
        lib.attrNames
      ];
      loadMachine =
        name:
        let
          path = dir + "/${name}";
          machineArgs = {
            inherit flakes;
            mkMachine = cfg.lib.mkMachine { inherit name path; };
          };
        in
        import path machineArgs;
    in
    lib.genAttrs machineNames loadMachine;
in
{
  inherit wat;
}
