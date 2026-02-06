{ flakes ? {}
, extraOverlays ? []
, extraModules ? []
}:

{ name
, path ? null
}:

{ nixpkgs ? flakes.nixpkgs
, system ? "x86_64-linux"
, loadModules ? []
}:

module:

let

  lib = nixpkgs.lib;

  availableModules = extraModules ++ loadModules;

  lookupMachineFiles = dir: prefix: let
    dirContents = builtins.readDir (dir + "/${prefix}");
    fileNames = lib.attrNames dirContents;
    toFileList = key:
      if dirContents.${key} == "regular"
        then lib.singleton (lib.nameValuePair (prefix + key) (dir + "/${prefix}${key}"))
        else if dirContents.${key} == "directory"
          then lookupMachineFiles dir ("${prefix}${key}/")
          else [];
    fileList = lib.concatMap toFileList fileNames;
  in fileList;

  machineFiles = if isNull path then {} else lib.listToAttrs (lookupMachineFiles path "");

  baseConfiguration = { config, lib, ... }: {
    nixpkgs.overlays = extraOverlays;

    _module.args = {
      inherit flakes;
    };

    nix.nixPath = [ "nixpkgs=${nixpkgs}" ];
    nix.registry.nixpkgs.flake = nixpkgs;
    nix.registry.n.flake = nixpkgs;
    networking.hostName = lib.mkDefault name;
  };

in lib.nixosSystem {
  inherit system;
  modules = availableModules ++ [ baseConfiguration module ];
} // {
  watExtraOutput.machineFiles = machineFiles;
}
