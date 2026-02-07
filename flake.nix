{

  outputs = inputs: import ./nix inputs;

  inputs.nixpkgs = {};

  inputs.dependencyDagOfSubmodule = {
    url = "github:thelegy/nix-dependencyDagOfSubmodule";
    inputs.nixpkgs.follows = "nixpkgs";
  };

}
