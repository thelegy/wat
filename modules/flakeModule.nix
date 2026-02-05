{ self, ... }:
{
  config,
  flake-parts-lib,
  inputs,
  lib,
  ...
}:
{

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

    enableAutoBuildTargets = lib.mkOption {
      type = lib.types.bool;
      default = true;
    };
    extraBuildTargets = lib.mkOption {
      type = lib.types.listOf lib.types.path;
      default = [ ];
    };

    outputs = lib.mkOption {
      type = lib.types.functionTo lib.types.attrs;
    };
  };

  config =
    let

      cfg = config.wat;

      watFlake =
        self.lib.mkWatRepo inputs
          {
            inherit (cfg)
              namespace
              namespacePrefix
              repoUuid
              dontLoadFlakeModules
              dontLoadWatModules
              loadModules
              dontLoadFlakeOverlay
              dontLoadWatOverlay
              loadOverlays
              enableAutoBuildTargets
              extraBuildTargets
              ;
          }
          (
            {
              findModules,
              findMachines,
              ...
            }:
            cfg.outputs {
              findModules = findModules cfg.namespace;
              inherit findMachines;
            }
          );

    in
    {

      flake = {
        inherit (watFlake) nixosConfigurations nixosModules;
        overlays.default = watFlake.overlay;
      };

      perSystem =
        { system, ... }:
        {
          devShells.default = watFlake.devShells.${system}.default;
          packages = {
            inherit (watFlake.packages.${system}) default prebuild-script wat-deploy-tools;
          };
        };

    };

}
