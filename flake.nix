{

  outputs = flakes@{ self, nixpkgs, ... }: let

    lib = import ./lib flakes;

  in {

    inherit lib;

    nixosModules = import ./modules flakes;

    overlay = import ./pkgs flakes;

    packages = (lib.eachDefaultSystem (system: pkgs: {
      systemOverlays = [ self.overlay ];
      packages = {
        inherit (pkgs) wat-install-activation;
      };
    })).packages;

  };

}
