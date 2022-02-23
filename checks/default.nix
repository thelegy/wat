{ self, ... }:
pkgs:
with pkgs.lib;

{

  dependencyDagOfSubmoduleTests = pkgs.callPackage ./dependencyDagOfSubmoduleTests.nix { watLib = self.lib.bake pkgs.lib; };

}
