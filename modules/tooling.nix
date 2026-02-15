{
  config,
  inputs,
  lib,
  ...
}:

let
  cfg = config.wat;
in
{

  options.wat = {
    enableAutoBuildTargets = lib.mkOption {
      type = lib.types.bool;
      default = true;
    };
    extraBuildTargets = lib.mkOption {
      type = lib.types.listOf lib.types.path;
      default = [ ];
    };
  };

  config.perSystem =
    { system, ... }:
    let
      pkgs = import inputs.nixpkgs {
        inherit system;
        overlays = [ inputs.wat.overlays.default ];
      };
    in
    {

      packages = {
        inherit (pkgs) wat-deploy-tools;
        prebuild-script = pkgs.wat-prebuild-script.override {
          inherit (cfg) enableAutoBuildTargets extraBuildTargets;
          selfFlake = inputs.self;
        };
      };

      devShells.default = pkgs.mkShellNoCC {
        name = "wat";
        packages = [
          pkgs.wat-deploy-tools
        ];
      };

    };

}
