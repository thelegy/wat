toplevel@{ self, ... }:
{
  config,
  flake-parts-lib,
  inputs,
  lib,
  ...
}:
{

  imports = [
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

    outputs = lib.mkOption {
      type = lib.types.functionTo lib.types.attrs;
    };
  };

  config.flake =
    let

      cfg = config.wat;
      flakes = inputs;

      repoUuidModule =
        { wat-installer-lib, ... }:
        {
          wat.installer.repoUuid =
            if !isNull cfg.repoUuid then
              cfg.repoUuid
            else
              (lib.foldl' (
                namespace: name: wat-installer-lib.uuidgen { inherit namespace name; }
              ) "59d93334-df87-4242-ac91-9c48886b4d94" cfg.namespace);
        };

      extraOverlays =
        cfg.loadOverlays
        ++ (lib.optionals (!cfg.dontLoadFlakeOverlay) (lib.toList (flakes.self.overlay or [ ])))
        ++ (lib.optionals (!cfg.dontLoadFlakeOverlay) (lib.toList (flakes.self.overlays.default or [ ])))
        ++ (lib.optionals (!cfg.dontLoadWatOverlay) (lib.toList (self.overlay or [ ])))
        ++ (lib.optionals (!cfg.dontLoadWatOverlay) (lib.toList (self.overlays.default or [ ])));

      extraModules =
        cfg.loadModules
        ++ (lib.optionals (!cfg.dontLoadWatModules) (lib.attrValues self.nixosModules))
        ++ (lib.optionals (!cfg.dontLoadWatModules) [ repoUuidModule ])
        ++ (lib.optionals (!cfg.dontLoadFlakeModules) (lib.attrValues (flakes.self.nixosModules or { })));

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
            self.lib.wrapModules lib {
              path = dir + "/${name}";
              namespace = cfg.namespacePrefix ++ cfg.namespace;
            }
          )
        );

      findMachines =
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
                mkMachine = self.lib.mkMachine { inherit flakes extraOverlays extraModules; } {
                  inherit name path;
                };
              };
            in
            import path machineArgs;
        in
        lib.genAttrs machineNames loadMachine;

      watFlake = cfg.outputs {
        inherit findModules findMachines;
      };

    in
    {
      inherit (watFlake) nixosConfigurations nixosModules;
      overlays.default = watFlake.overlay;
    };

}
