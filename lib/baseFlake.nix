{ self, ... }:
with self.lib;

nixpkgs:
with nixpkgs.lib;

let
  systems = platforms.linux;
in rec {

  packages = withPkgsFor systems nixpkgs [ self.overlay ] (pkgs: rec {
    inherit (pkgs) wat-deploy-tools wat-deploy-tools-envrc;
    default = wat-deploy-tools;
  });

}
