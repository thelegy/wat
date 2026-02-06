{ wat, ... }:
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
        wat.lib.wrapModules lib {
          path = dir + "/${name}";
          namespace = cfg.namespacePrefix ++ cfg.namespace;
        }
      )
    );
in
{
  wat.lib = { inherit findModules; };
}
