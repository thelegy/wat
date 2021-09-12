{ self, nixpkgs, ... }:
with self.lib;


eachDefaultSystem (system: pkgs: rec {

  systemOverlays = [ self.overlay ];

  packages = {
    inherit (pkgs)
      wat-deploy-tools
      wat-deploy-tools-envrc
      ;
  };

  legacyPackages = {
    inherit (pkgs)
      wat-install-helpers
      ;
  };

  defaultPackage = packages.wat-deploy-tools;

})
