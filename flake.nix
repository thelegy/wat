{

  outputs = flakes@{ self, nixpkgs, ... }: {

    lib = import ./lib flakes;

    nixosModules = {};

  };

}
