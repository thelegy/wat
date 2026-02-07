{ writeScriptBin
, lib
, python3
, enableAutoBuildTargets ? false
, extraBuildTargets ? []
, selfFlake ? "."
}: with lib;

let
  autoBuildTargets = map (x: "nixosConfigurations.${x}.config.system.build.toplevel") (attrNames selfFlake.nixosConfigurations);
  buildTargets = (optionals enableAutoBuildTargets autoBuildTargets) ++ extraBuildTargets;
in writeScriptBin "wat-prebuild-script" ''
  #!/bin/sh
  export FLAKE=${escapeShellArg selfFlake}
  export TARGETS=${concatMapStringsSep ":" escapeShellArg buildTargets}

  exec ${python3}/bin/python ${./script.py} "$@"
''
