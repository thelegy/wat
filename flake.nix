{

  outputs = flakes@{ self, nixpkgs, ... }: {

    lib = import ./lib.nix flakes;

    baseFlake = import ./baseFlake.nix flakes;

  };

}
