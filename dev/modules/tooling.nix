{ inputs, wat, ... }:
{

  flake-file.inputs.crane.url = "github:ipetkov/crane";

  perSystem =
    { config, pkgs, ... }:
    let
      craneLib = inputs.crane.mkLib pkgs;
      commonArgs = {
        src = craneLib.cleanCargoSource wat;
        strictDeps = true;
      };
      wat-tools = craneLib.buildPackage (
        commonArgs
        // {
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          nativeBuildInputs = (commonArgs.nativeBuildInputs or [ ]) ++ [ pkgs.installShellFiles ];
          postInstall = ''
            installShellCompletion --cmd wat --bash <($out/bin/wat completion bash)
            installShellCompletion --cmd wat --zsh <($out/bin/wat completion zsh)
            installShellCompletion --cmd wat --fish <($out/bin/wat completion fish)
          '';
        }
      );
      checks = { inherit wat-tools; };
      packages.default = wat-tools;
      devShells.default = craneLib.devShell {
        inputsFrom = [
          wat-tools
          config.pre-commit.devShell
        ];
        packages = [
          pkgs.rust-analyzer
          pkgs.rustfmt
        ];
      };
    in
    {
      inherit checks devShells packages;
    };

}
