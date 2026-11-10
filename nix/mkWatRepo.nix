# Standalone adapter for flakes that do not use flake-parts.
watInputs: lib: repoInputs: makeRepo:
let
  inputs = repoInputs // {
    # The shared modules use this internal alias; callers may name the input freely.
    wat = watInputs.self;
  };

  # Supply the system-specific arguments used by mkModule without evaluating a
  # second flake framework. Non-system outputs remain available through inputs.
  prime =
    system: flake:
    lib.genAttrs (builtins.filter (name: builtins.hasAttr name flake) [
      "packages"
      "legacyPackages"
      "apps"
      "checks"
      "devShells"
      "formatter"
    ]) (name: flake.${name}.${system});
  withSystem =
    system: f:
    f {
      inherit system;
      pkgs = import inputs.nixpkgs { inherit system; };
      inputs' = lib.mapAttrs (_: prime system) inputs;
      self' = prime system inputs.self;
    };

  settings = builtins.removeAttrs (import ../modules/options.nix { inherit lib; }).options.wat [
    "lib"
    "build"
  ];
  unknown = builtins.attrNames (
    builtins.removeAttrs repo ([ "outputs" ] ++ builtins.attrNames settings)
  );

  framework = lib.evalModules {
    specialArgs = { inherit inputs withSystem; };
    modules = (import ./repoModules.nix) ++ [
      {
        # Keep the module structure independent of the callback result so the
        # callback can use helpers that themselves depend on these settings.
        wat =
          lib.mapAttrs (name: _: lib.mkIf (builtins.hasAttr name repo) repo.${name}) (
            builtins.removeAttrs settings [ "namespace" ]
          )
          // {
            namespace = repo.namespace or [ ];
          };
      }
    ];
  };
  repo = makeRepo framework.config.wat.lib;
in
if unknown != [ ] then
  throw "wat.mkWatRepo: unknown settings: ${lib.concatStringsSep ", " unknown}"
else
  repo.outputs or { }
