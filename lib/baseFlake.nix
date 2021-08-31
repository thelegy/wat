{ self, ... }:
with self.lib;


eachDefaultSystem (system: pkgs: rec {

  packages.wat-deploy-tools = pkgs.callPackage ../wat-deploy-tools {};

  packages.wat-deploy-tools-envrc = pkgs.writeText "wat-deploy-tools-envrc" ''
    PATH_add ${packages.wat-deploy-tools}/bin
  '';

  defaultPackage = packages.wat-deploy-tools;

})
