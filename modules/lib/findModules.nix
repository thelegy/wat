{ config, lib, ... }:
let
  cfg = config.wat;

  findModules =
    dir:
    let
      moduleNames = lib.pipe dir [
        builtins.readDir
        (lib.filterAttrs (
          key: val: !lib.hasPrefix "." key && (lib.hasSuffix ".nix" key || val == "directory")
        ))
        lib.attrNames
      ];
    in
    lib.listToAttrs (
      lib.forEach moduleNames (
        name:
        cfg.build.wrapModule {
          path = dir + "/${name}";
        }
      )
    );
in
{
  wat.lib = { inherit findModules; };
}
