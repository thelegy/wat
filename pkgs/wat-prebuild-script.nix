{ writeScriptBin
, lib
, zsh
, enableAutoBuildTargets ? false
, extraBuildTargets ? []
, selfFlake ? "."
}: with lib;

let
  autoBuildTargets = map (x: "nixosConfigurations.${x}.config.system.build.toplevel") (attrNames selfFlake.nixosConfigurations);
  buildTargets = (optionals enableAutoBuildTargets autoBuildTargets) ++ extraBuildTargets;
in writeScriptBin "wat-prebuild-script" ''
  #!${zsh}/bin/zsh
  set -euo pipefail

  readonly flake=${escapeShellArg selfFlake}
  readonly targets=(${escapeShellArgs buildTargets})
  jobs=()
  exitCode=0

  buildTarget() {
    set +e
    target=$1
    nix path-info --derivation $flake#$target >&- 2>&-
    if [[ $? != 0 ]] {
      echo "$target did not evaluate" >&2
      exit 2
    }
    nix build --no-link $flake#$target >&- 2>&-
    if [[ $? != 0 ]] {
      echo "$target did not build" >&2
      exit 1
    }
    echo "$target was built successfully" >&2
  }

  for target in $targets; {
    buildTarget $target &
    jobs+=$!
  }

  for job in $jobs; {
    wait $job || exitCode=$(( $exitCode > $? ? $exitCode : $? ))
  }

  exit $exitCode
''
