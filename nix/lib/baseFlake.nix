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
  overlays = (toList (self.overlay or [])) ++ (toList (self.overlays.default or []));
in {

  packages = withPkgsFor systems nixpkgs overlays (pkgs: rec {
    inherit (pkgs) wat-deploy-tools;
    prebuild-script = pkgs.wat-prebuild-script.override {
      inherit enableAutoBuildTargets extraBuildTargets selfFlake;
    };
    default = wat-deploy-tools;
  });

  devShells = withPkgsFor systems nixpkgs overlays (pkgs: {
    default = pkgs.mkShell {
      packages = [
        pkgs.wat-deploy-tools
      ];
    };
  });

}
