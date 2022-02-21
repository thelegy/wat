{

  outputs = flakes@{ nixpkgs, ... }: rec {

    lib = import ./lib flakes;

    nixosModules = import ./modules flakes;

    overlay = import ./pkgs flakes;

    checks = lib.withPkgsFor [ "x86_64-linux" ] nixpkgs [ overlay ] (import ./checks flakes);

  };

}
