{ lib, pkgs, symlinkJoin }:
with lib;

let

  extendedPackages = pkgs.extend  (final: prev: with final; {
    wat-deploy = callPackage ./deploy.nix {};
    wat-update-envrc = callPackage ./update-envrc.nix {};
  });

in symlinkJoin {
  name = "wat-deploy-tools";
  paths = with extendedPackages; [
    wat-deploy
    wat-update-envrc
  ];
}
