{

  outputs = inputs: import ./nix inputs;

  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

  inputs.dependencyDagOfSubmodule = {
    url = "github:thelegy/nix-dependencyDagOfSubmodule";
    inputs.nixpkgs.follows = "nixpkgs";
  };

}
