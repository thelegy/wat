watFlakes@{ self, ... }:

flakes@{ nixpkgs ? watFlakes.nixpkgs, ... }:
with self.lib.bake nixpkgs.lib;

fn:

let

  defaultArgs = {
    dontLoadFlakeModules = false;
    dontLoadWatModules = false;
    loadModules = [];

    dontLoadFlakeOverlay = false;
    dontLoadWatOverlay = false;
    loadOverlays = [];

    namespacePrefix = [ "wat" ];
    namespace = [];
    repoUuid = null;
  };

  repoGenFn = a: with a; let

    repoUuidModule = { wat-installer-lib, ... }: {
      wat.installer.repoUuid = if !isNull repoUuid then repoUuid else (foldl'
        (namespace: name: wat-installer-lib.uuidgen { inherit namespace name;})
        "59d93334-df87-4242-ac91-9c48886b4d94"
        namespace);
    };

    extraOverlays = loadOverlays
      ++ (optionals (!dontLoadFlakeOverlay) (toList (flakes.self.overlay or [])))
      ++ (optionals (!dontLoadWatOverlay) (toList (self.overlay or [])))
    ;

    extraModules = loadModules
      ++ (optionals (!dontLoadWatModules) (attrValues self.nixosModules))
      ++ (optionals (!dontLoadWatModules) [repoUuidModule])
      ++ (optionals (!dontLoadFlakeModules) (attrValues (flakes.self.nixosModules or {})))
    ;


  in fn {

    findModules = namespace: dir: let
      moduleNames = pipe dir [
        builtins.readDir
        (filterAttrs (key: val: ! hasPrefix "." key && (hasSuffix ".nix" key || val == "directory")))
        attrNames
      ];
    in listToAttrs (forEach moduleNames (name: mkModule {
      path = dir + "/${name}";
      namespace = namespacePrefix ++ namespace;
    }));

    findMachines = dir: let
      machineNames = pipe dir [
        builtins.readDir
        (filterAttrs (key: val: ! hasPrefix "." key && val == "directory"))
        attrNames
      ];
      loadMachine = name: let
        path = dir + "/${name}";
        machineArgs = {
          inherit flakes;
          mkMachine = mkMachine { inherit flakes extraOverlays extraModules; } { inherit name path; };
        };
      in import path machineArgs;
    in genAttrs machineNames loadMachine;

  };

  filterApplyDefaultArgs = fn: r: fn (mapAttrs (key: val: attrByPath [key] val r) defaultArgs);

  filterOutputs = flip pipe [
    (filterAttrs (key: val: !hasAttr key defaultArgs))
    ({ outputs }: outputs)
  ];

in pipe repoGenFn [
  filterApplyDefaultArgs
  fix
  filterOutputs
  (recursiveUpdate (baseFlake nixpkgs))
]
