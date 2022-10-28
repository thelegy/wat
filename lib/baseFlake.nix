{ self, ... }:
with self.lib;

{ nixpkgs
, enableAutoBuildTargets ? false
, extraBuildTargets ? []
, selfFlake
}:
with nixpkgs.lib;

let
  systems = platforms.linux;
in rec {

  packages = withPkgsFor systems nixpkgs [ self.overlay ] (pkgs: rec {
    inherit (pkgs) wat-deploy-tools wat-deploy-tools-envrc;
    prebuild-script = pkgs.wat-prebuild-script.override {
      inherit enableAutoBuildTargets extraBuildTargets selfFlake;
    };
    default = wat-deploy-tools;
  });

}
