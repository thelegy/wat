toplevel:
{
  config,
  flake-parts-lib,
  inputs,
  lib,
  ...
}:
{

  imports = [
    (import ./lib toplevel)
    (import ./tooling.nix toplevel)
  ];

  options.wat = {
    namespace = lib.mkOption {
      type = lib.types.listOf lib.types.str;
    };
    namespacePrefix = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ "wat" ];
    };
    repoUuid = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
    };

    dontLoadFlakeModules = lib.mkOption {
      type = lib.types.bool;
      default = false;
    };
    dontLoadWatModules = lib.mkOption {
      type = lib.types.bool;
      default = false;
    };
    loadModules = lib.mkOption {
      type = lib.types.listOf lib.types.deferredModule;
      default = [ ];
    };

    dontLoadFlakeOverlay = lib.mkOption {
      type = lib.types.bool;
      default = false;
    };
    dontLoadWatOverlay = lib.mkOption {
      type = lib.types.bool;
      default = false;
    };
    loadOverlays = lib.mkOption {
      type = lib.types.listOf lib.types.anything;
      default = [ ];
    };

    lib = lib.mkOption {
      type = lib.types.attrs;
    };

  };

}
