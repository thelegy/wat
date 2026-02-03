{

  outputs = inputs: inputs.flake-parts.lib.mkFlake { inherit inputs; } (inputs.import-tree ./modules);

  inputs.nixpkgs = {};

  inputs.flake-parts = {
    inputs.nixpkgs-lib.follows = "nixpkgs";
    url = "github:hercules-ci/flake-parts";
  };

  inputs.import-tree.url = "github:vic/import-tree";

  inputs.dependencyDagOfSubmodule = {
    url = "github:thelegy/nix-dependencyDagOfSubmodule";
    inputs.nixpkgs.follows = "nixpkgs";
  };

}
