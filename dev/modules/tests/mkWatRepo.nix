{ inputs, lib, ... }:
let
  mkWatRepo = inputs.wat.lib.mkWatRepo;
in
{
  flake.tests.mkWatRepo =
    let
      inputs = {
        self.packages.test-system.value = 2;
        nixpkgs = { inherit lib; };
        example.packages.test-system.value = 40;
      };
      result = mkWatRepo lib inputs (
        {
          toNamespace,
          fromNamespace,
          mkModule,
          hostModule,
          ...
        }:
        {
          namespace = [ "repo" ];
          dontLoadWatModules = true;
          dontLoadWatOverlay = true;
          outputs = {
            namespaced = toNamespace [ "feature" ] 42;
            extracted = fromNamespace [ "feature" ] { wat.repo.feature = 43; };
            module = mkModule [ "feature" ] { };
            overlays = hostModule.nixpkgs.overlays;
            primed =
              (lib.evalModules {
                modules = [
                  (mkModule [ "primed" ] (
                    { inputs', self', ... }:
                    {
                      config.value = inputs'.example.packages.value + self'.packages.value;
                    }
                  ))
                  {
                    config._module.args.pkgs.stdenv.hostPlatform.system = "test-system";
                    options.value = lib.mkOption { type = lib.types.int; };
                    config.wat.repo.primed.enable = true;
                  }
                ];
              }).config.value;
          };
        }
      );
    in
    {
      testNamespace = {
        expr = result.namespaced;
        expected.wat.repo.feature = 42;
      };
      testExtraction = {
        expr = result.extracted;
        expected = 43;
      };
      testModule = {
        expr = lib.isFunction result.module;
        expected = true;
      };
      testNoOverlays = {
        expr = result.overlays;
        expected = [ ];
      };
      testOverlayFinalPackageSet = {
        expr =
          let
            repo = mkWatRepo lib inputs (
              { hostModule, ... }:
              {
                dontLoadWatModules = true;
                dontLoadWatOverlay = true;
                loadOverlays = [
                  (final: prev: {
                    answer = final.base + prev.increment;
                  })
                ];
                outputs.overlays = hostModule.nixpkgs.overlays;
              }
            );
            pkgs = lib.fix (
              lib.extends (lib.composeManyExtensions repo.overlays) (_: {
                base = 40;
                increment = 2;
              })
            );
          in
          pkgs.answer;
        expected = 42;
      };
      testSystemArguments = {
        expr = result.primed;
        expected = 42;
      };
      testUnknownSettings = {
        expr =
          (builtins.tryEval (
            mkWatRepo lib inputs (_: {
              typo = true;
            })
          )).success;
        expected = false;
      };
      testEmptyOutputs = {
        expr = mkWatRepo lib inputs (_: { });
        expected = { };
      };
    };
}
