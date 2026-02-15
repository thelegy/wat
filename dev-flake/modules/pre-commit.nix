{ inputs, lib, ... }:
{
  imports = [
    (inputs.git-hooks-nix.flakeModule or { })
  ];

  flake-file.inputs.git-hooks-nix.url = "github:cachix/git-hooks.nix";
  flake-file.inputs.git-hooks-nix.inputs.nixpkgs.follows = "nixpkgs";

  perSystem =
    { config, ... }:
    {
      pre-commit.settings = {
        rootSrc = lib.mkForce ../..;

        hooks.deadnix.enable = true;
        hooks.nil.enable = true;
      };

      devShells.pre-commit = config.pre-commit.devShell;
    };
}
