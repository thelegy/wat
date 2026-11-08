# Run with: nix-instantiate --eval --strict tests/module-helpers.nix --arg lib '(import <nixpkgs> {}).lib'
{ lib }:
let
  framework = lib.evalModules {
    specialArgs.withSystem =
      system: f:
      f {
        inputs'.example.value = if system == "aarch64-linux" then 40 else 10;
        self'.value = 2;
      };
    modules = [
      ../modules/options.nix
      ../modules/lib/mkModule.nix
      ../modules/lib/wrapModule.nix
      { wat.namespace = [ "repo" ]; }
    ];
  };
  wat = framework.config.wat.lib;
  evaluate =
    class: enabled: modules:
    lib.evalModules {
      inherit class;
      specialArgs.token = 17;
      modules = [
        {
          options.result = lib.mkOption {
            type = lib.types.int;
            default = 0;
          };
          options.programs.known = lib.mkOption {
            type = lib.types.bool;
            default = false;
          };
        }
        { config = if enabled == null then { } else wat.toNamespace [ "feature" ] { enable = enabled; }; }
      ]
      ++ modules;
    };
  full = wat.mkModule [ "feature" ] (
    {
      cfg,
      token,
      config,
      ...
    }:
    {
      options.amount = lib.mkOption {
        type = lib.types.int;
        default = token;
      };
      config.result = cfg.amount + (if config.wat.repo.feature.enable then 1 else 0);
    }
  );
  legacyOrderingModule =
    (framework.config.wat.build.wrapModule {
      path = ./fixtures/legacy-ordering-module.nix;
      name = "feature";
    }).value;
  ordered =
    module:
    (evaluate "nixos" true [
      {
        options.ordered = lib.mkOption { type = lib.types.listOf lib.types.str; };
        config.ordered = [ "before" ];
      }
      module
      { ordered = [ "after" ]; }
    ]).config.ordered;
  migratedOrderingModule = {
    ordered = [ "migrated" ];
  };
  throws = value: !(builtins.tryEval (builtins.deepSeq value true)).success;
  tests = lib.runTests {
    testWrappedPreservesLegacyOrdering = {
      expr = ordered (wat.mkModuleWrapped [ "feature" ] migratedOrderingModule);
      expected = ordered legacyOrderingModule;
    };
    testDirectModuleChangesOrdering = {
      expr = ordered (wat.mkModule [ "feature" ] migratedOrderingModule) != ordered legacyOrderingModule;
      expected = true;
    };
    testWrappedModuleArguments = {
      expr =
        (evaluate "nixos" true [
          { _module.args.answer = 42; }
          (wat.mkModuleWrapped [ "feature" ] (
            { answer, cfg, ... }:
            {
              options.amount = lib.mkOption { default = answer; };
              config.result = cfg.amount;
            }
          ))
        ]).config.result;
      expected = 42;
    };
    testWrappedLocation = {
      expr = (wat.mkModuleWrapped [ "feature" ] { })._file;
      expected = "wat.mkModuleWrapped wat.repo.feature";
    };
    testWrappedDefinitionLocation = {
      expr =
        map (definition: definition.file)
          (evaluate "nixos" true [
            (wat.mkModuleWrapped [ "feature" ] {
              _file = "migrated-module.nix";
              result = 4;
            })
          ]).options.result.definitionsWithLocations;
      expected = [ "migrated-module.nix" ];
    };
    testWrappedDisabled = {
      expr =
        (evaluate "nixos" false [
          (wat.mkModuleWrapped [ "feature" ] { result = 4; })
        ]).config.result;
      expected = 0;
    };
    testDisabledByDefault = {
      expr = (evaluate "nixos" null [ full ]).config.wat.repo.feature.enable;
      expected = false;
    };
    testModuleArguments = {
      expr =
        (evaluate "nixos" true [
          { _module.args.answer = 42; }
          (wat.mkModule [ "feature" ] (
            { answer, ... }:
            {
              result = answer;
            }
          ))
        ]).config.result;
      expected = 42;
    };
    testPrimedArguments = {
      expr =
        map
          (
            class:
            (evaluate class true [
              { _module.args.pkgs.stdenv.hostPlatform.system = "aarch64-linux"; }
              (wat.mkModule [ "feature" ] (
                {
                  inputs',
                  self',
                  cfg,
                  ...
                }:
                {
                  options.amount = lib.mkOption { default = inputs'.example.value; };
                  config.result = cfg.amount + self'.value;
                }
              ))
            ]).config.result
          )
          [
            "nixos"
            "homeManager"
          ];
      expected = [
        42
        42
      ];
    };
    testLegacyWrapper = {
      expr =
        (evaluate "nixos" true [
          (framework.config.wat.build.wrapModule {
            path = ./fixtures/legacy-module.nix;
            name = "feature";
          }).value
        ]).config.result;
      expected = 23;
    };
    testNamespace = {
      expr = wat.namespacePath [
        "a"
        "b"
      ];
      expected = [
        "wat"
        "repo"
        "a"
        "b"
      ];
    };
    testNamespaceRoundTrip = {
      expr = wat.fromNamespace [ "a" ] (wat.toNamespace [ "a" ] { value = 3; });
      expected = {
        value = 3;
      };
    };
    testInvalidNamespace = {
      expr = throws (wat.namespacePath [ 1 ]);
      expected = true;
    };
    testNixos = {
      expr = (evaluate "nixos" true [ full ]).config.result;
      expected = 18;
    };
    testHomeManager = {
      expr = (evaluate "homeManager" true [ full ]).config.result;
      expected = 18;
    };
    testDisabled = {
      expr = (evaluate "nixos" false [ full ]).config.result;
      expected = 0;
    };
    testOptionsWhileDisabled = {
      expr = (evaluate "nixos" false [ full ]).config.wat.repo.feature.amount;
      expected = 17;
    };
    testShorthand = {
      expr = (evaluate "nixos" true [ (wat.mkModule [ "feature" ] { result = 4; }) ]).config.result;
      expected = 4;
    };
    testFunctionShorthand = {
      expr =
        (evaluate "homeManager" true [
          (wat.mkModule [ "feature" ] (
            { token, ... }:
            {
              result = token;
            }
          ))
        ]).config.result;
      expected = 17;
    };
    testUnknownOptionWhileDisabled = {
      expr =
        (evaluate "nixos" false [ (wat.mkModule [ "feature" ] { programs.nonexistent = true; }) ])
        .config.result;
      expected = 0;
    };
    testUnknownOptionWhileEnabled = {
      expr =
        throws
          (evaluate "nixos" true [ (wat.mkModule [ "feature" ] { programs.nonexistent = true; }) ]).config;
      expected = true;
    };
    testSelfEnable = {
      expr =
        throws
          (evaluate "nixos" false [
            (wat.mkModule [ "feature" ] (wat.toNamespace [ "feature" ] { enable = true; }))
          ]).config;
      expected = true;
    };
    testImportsRejectedInShorthand = {
      expr =
        throws
          (evaluate "nixos" false [
            (wat.mkModule [ "feature" ] {
              imports = [ { result = 9; } ];
              result = 1;
            })
          ]).config;
      expected = true;
    };
    testImportsRejectedInExplicitFunction = {
      expr =
        throws
          (evaluate "homeManager" true [
            (wat.mkModule [ "feature" ] (_: {
              imports = [ { result = 9; } ];
              config.result = 1;
            }))
          ]).config;
      expected = true;
    };
    testLegacyImportsRejected = {
      expr =
        throws
          (evaluate "nixos" false [
            (wat.mkModule [ "feature" ] { require = [ { result = 9; } ]; })
          ]).config;
      expected = true;
    };
    testMergeProperties = {
      expr =
        (evaluate "nixos" true [
          (wat.mkModule [ "feature" ] {
            config = lib.mkMerge [
              (lib.mkIf true { result = lib.mkDefault 6; })
              { result = 7; }
            ];
          })
        ]).config.result;
      expected = 7;
    };
    testLiftedConfig = {
      expr =
        (evaluate "nixos" true [
          (wat.mkModule [ "feature" ] {
            options.amount = lib.mkOption {
              type = lib.types.int;
              default = 0;
            };
            config = wat.toNamespace [ "feature" ] { amount = 8; };
          })
        ]).config.wat.repo.feature.amount;
      expected = 8;
    };
    testSeparateCfgArguments = {
      expr =
        (evaluate "nixos" true [
          full
          (wat.mkModule [ "other" ] (
            { cfg, ... }:
            {
              options.amount = lib.mkOption { default = 99; };
              config.otherResult = cfg.amount;
            }
          ))
          {
            options.otherResult = lib.mkOption { type = lib.types.int; };
            config = wat.toNamespace [ "other" ] { enable = true; };
          }
        ]).config.otherResult;
      expected = 99;
    };
    testWatArgument = {
      expr =
        (lib.evalModules {
          modules = [
            ../modules/options.nix
            ../modules/lib/mkModule.nix
            { wat.namespace = [ "repo" ]; }
            (
              { wat, ... }:
              {
                wat.repoUuid = lib.concatStringsSep "." (wat.namespacePath [ "feature" ]);
              }
            )
          ];
        }).config.wat.repoUuid;
      expected = "wat.repo.feature";
    };
  };
in
assert lib.assertMsg (tests == [ ]) (builtins.toJSON tests);
true
