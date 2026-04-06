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
            installShellCompletion --cmd wat --bash <(COMPLETE="bash" $out/bin/wat)
            installShellCompletion --cmd wat --zsh <(COMPLETE="zsh" $out/bin/wat)
            installShellCompletion --cmd wat --fish <(COMPLETE="fish" $out/bin/wat)
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
