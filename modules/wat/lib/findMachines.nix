{ self, wat, ... }:
{
  config,
  inputs,
  lib,
  ...
}:
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
            mkMachine = wat.lib.mkMachine { inherit flakes extraOverlays extraModules; } {
              inherit name path;
            };
          };
        in
        import path machineArgs;
    in
    lib.genAttrs machineNames loadMachine;
in
{
  wat.lib = { inherit findMachines; };
}
