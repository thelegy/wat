watFlakes@{ self, ... }:

flakes@{
  nixpkgs ? watFlakes.nixpkgs,
  ...
}:

{
  dontLoadFlakeModules ? false,
  dontLoadWatModules ? false,
  loadModules ? [ ],

  dontLoadFlakeOverlay ? false,
  dontLoadWatOverlay ? false,
  loadOverlays ? [ ],

  namespacePrefix ? [ "wat" ],
  namespace ? [ ],
  repoUuid ? null,

  enableAutoBuildTargets ? true,
  extraBuildTargets ? [ ],
}:

outputsFn:

let

  lib = nixpkgs.lib;

  repoUuidModule =
    { wat-installer-lib, ... }:
    {
      wat.installer.repoUuid =
        if !isNull repoUuid then
          repoUuid
        else
          (lib.foldl' (
            namespace: name: wat-installer-lib.uuidgen { inherit namespace name; }
          ) "59d93334-df87-4242-ac91-9c48886b4d94" namespace);
    };

  extraOverlays =
    loadOverlays
    ++ (lib.optionals (!dontLoadFlakeOverlay) (lib.toList (flakes.self.overlay or [ ])))
    ++ (lib.optionals (!dontLoadFlakeOverlay) (lib.toList (flakes.self.overlays.default or [ ])))
    ++ (lib.optionals (!dontLoadWatOverlay) (lib.toList (self.overlay or [ ])))
    ++ (lib.optionals (!dontLoadWatOverlay) (lib.toList (self.overlays.default or [ ])));

  extraModules =
    loadModules
    ++ (lib.optionals (!dontLoadWatModules) (lib.attrValues self.nixosModules))
    ++ (lib.optionals (!dontLoadWatModules) [ repoUuidModule ])
    ++ (lib.optionals (!dontLoadFlakeModules) (lib.attrValues (flakes.self.nixosModules or { })));

  baseFlakeArgs = {
    inherit enableAutoBuildTargets extraBuildTargets;
    inherit nixpkgs;
    selfFlake = flakes.self;
  };

  extraResults = self.lib.baseFlake baseFlakeArgs;

in
lib.recursiveUpdate extraResults (outputsFn {

  findModules =
    namespace: dir:
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
        (self.lib.bake lib).wrapModules {
          path = dir + "/${name}";
          namespace = namespacePrefix ++ namespace;
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
})
