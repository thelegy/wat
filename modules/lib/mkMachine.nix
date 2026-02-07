{ self, ... }:
{
  config,
  inputs,
  ...
}:
let
  cfg = config.wat;
  flakes = inputs;
  wat.lib.mkMachine =
    {
      name,
      path ? null,
    }:
    {
      nixpkgs ? inputs.nixpkgs,
      system ? "x86_64-linux",
      loadModules ? [ ],
    }:
    module:
    let
      lib = nixpkgs.lib;

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

      availableModules = extraModules ++ loadModules;

      lookupMachineFiles =
        dir: prefix:
        let
          dirContents = builtins.readDir (dir + "/${prefix}");
          fileNames = lib.attrNames dirContents;
          toFileList =
            key:
            if dirContents.${key} == "regular" then
              lib.singleton (lib.nameValuePair (prefix + key) (dir + "/${prefix}${key}"))
            else if dirContents.${key} == "directory" then
              lookupMachineFiles dir ("${prefix}${key}/")
            else
              [ ];
          fileList = lib.concatMap toFileList fileNames;
        in
        fileList;

      machineFiles = if isNull path then { } else lib.listToAttrs (lookupMachineFiles path "");

      baseConfiguration =
        { config, lib, ... }:
        {
          nixpkgs.overlays = extraOverlays;

          _module.args = {
            inherit flakes;
          };

          nix.nixPath = [ "nixpkgs=${nixpkgs}" ];
          nix.registry.nixpkgs.flake = nixpkgs;
          nix.registry.n.flake = nixpkgs;
          networking.hostName = lib.mkDefault name;
        };

    in
    nixpkgs.lib.nixosSystem {
      inherit system;
      modules = availableModules ++ [
        baseConfiguration
        module
      ];
    }
    // {
      watExtraOutput.machineFiles = machineFiles;
    };
in
{
  inherit wat;
}
