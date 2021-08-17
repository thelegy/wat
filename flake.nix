{

  outputs = flakes@{ self, nixpkgs, ... }: {

    lib = import ./lib flakes;

    baseFlake = import ./baseFlake.nix flakes;

  };

}
