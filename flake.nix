{

  outputs = flakes@{ nixpkgs, ... }: rec {

    lib = import ./lib flakes;

    nixosModules = import ./modules flakes;

    overlay = import ./pkgs flakes;

    packages = lib.withPkgsForLinux nixpkgs [ overlay ] (pkgs: {
      inherit (pkgs) wat-install-activation;
    });

  };

}
