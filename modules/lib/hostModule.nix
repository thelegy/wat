{
  config,
  inputs,
  lib,
  ...
}:
let
  cfg = config.wat;
  flakes = inputs;

  extraOverlays =
    cfg.loadOverlays
    ++ (lib.optionals (!cfg.dontLoadFlakeOverlay) (lib.toList (inputs.self.overlay or [ ])))
    ++ (lib.optionals (!cfg.dontLoadFlakeOverlay) (lib.toList (inputs.self.overlays.default or [ ])))
    ++ (lib.optionals (!cfg.dontLoadWatOverlay) (lib.toList (inputs.wat.overlay or [ ])))
    ++ (lib.optionals (!cfg.dontLoadWatOverlay) (lib.toList (inputs.wat.overlays.default or [ ])));

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

  extraModules =
    cfg.loadModules
    ++ (lib.optionals (!cfg.dontLoadWatModules) (lib.attrValues inputs.wat.nixosModules))
    ++ (lib.optionals (!cfg.dontLoadWatModules) [ repoUuidModule ])
    ++ (lib.optionals (!cfg.dontLoadFlakeModules) (lib.attrValues (inputs.self.nixosModules or { })));

  wat.lib.hostModule =
    { config, lib, ... }:
    {
      imports = extraModules;

      nixpkgs.overlays = extraOverlays;

      _module.args = {
        inherit flakes;
      };
    };
in
{
  inherit wat;
}
