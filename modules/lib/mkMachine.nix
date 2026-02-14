{
  config,
  inputs,
  ...
}:
let
  cfg = config.wat;
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

      watExtraOutput.machineFiles =
        if isNull path then { } else lib.listToAttrs (lookupMachineFiles path "");

      baseConfiguration =
        { config, lib, ... }:
        {
          nix.nixPath = [ "nixpkgs=${nixpkgs}" ];
          nix.registry.nixpkgs.flake = nixpkgs;
          nix.registry.n.flake = nixpkgs;
          networking.hostName = lib.mkDefault name;
        };

    in
    nixpkgs.lib.nixosSystem {
      inherit system;
      modules = loadModules ++ [
        cfg.lib.hostModule
        baseConfiguration
        module
      ];
    }
    // {
      inherit watExtraOutput;
    };
in
{
  inherit wat;
}
