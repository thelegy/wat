{ self, ... }:

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
with self.lib.bake nixpkgs.lib;

module:

let

  availableModules = extraModules ++ loadModules;

  lookupMachineFiles = dir: prefix: let
    dirContents = builtins.readDir (dir + "/${prefix}");
    fileNames = attrNames dirContents;
    toFileList = key:
      if dirContents.${key} == "regular"
        then singleton (nameValuePair (prefix + key) (dir + "/${prefix}${key}"))
        else if dirContents.${key} == "directory"
          then lookupMachineFiles dir ("${prefix}${key}/")
          else [];
    fileList = concatMap toFileList fileNames;
  in fileList;

  machineFiles = if isNull path then {} else listToAttrs (lookupMachineFiles path "");

  baseConfiguration = { config, lib, ... }: {
    nixpkgs.overlays = extraOverlays;

    _module.args = {
      inherit flakes availableModules;
      watLib = self.lib.bake lib;
    };

    nix.nixPath = [ "nixpkgs=${nixpkgs}" ];
    nix.registry.nixpkgs.flake = nixpkgs;
    nix.registry.n.flake = nixpkgs;
    networking.hostName = mkDefault name;
  };

in nixpkgs.lib.nixosSystem {
  inherit system;
  modules = availableModules ++ [ baseConfiguration module ];
} // {
  watExtraOutput.machineFiles = machineFiles;
}
