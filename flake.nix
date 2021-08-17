{

  outputs = flakes@{ self, nixpkgs, ... }: {

    lib = import ./lib.nix flakes;

  };

}
