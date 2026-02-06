{ lib
, callPackage
, wat
, writeShellScriptBin
}: with lib;

writeShellScriptBin "wat-gather" ''
  hostName="$1"
  system="$(ssh "$hostName" -- uname -m)-linux"


  nix copy --to "ssh://$hostName" "${wat}#legacyPackages.$system.wat-gather-script"

  ssh "$hostName" -- "$(nix path-info "${wat}#legacyPackages.$system.wat-gather-script")"
''
