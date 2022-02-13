{ self, ... }:
with self.lib;

nixpkgs:
with nixpkgs.lib;

let
  systems = platforms.linux;
in rec {

  packages = withPkgsFor systems nixpkgs [ self.overlay ] (pkgs: {
    inherit (pkgs) wat-deploy-tools wat-deploy-tools-envrc;
  });

  defaultPackage = genAttrs systems (system: packages.${system}.wat-deploy-tools);

}
