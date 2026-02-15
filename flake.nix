{

  outputs = inputs: import ./nix inputs;

  inputs.dependencyDagOfSubmodule.url = "github:thelegy/nix-dependencyDagOfSubmodule/dev";

}
