{ writeScriptBin, wat, zsh, nix-remote-run, nix-with-flakes, jq }:

writeScriptBin "wat-install" ''
  #!${zsh}/bin/zsh
  set -e -u -o pipefail

  if [[ -n "''${WAT_DO_FORMAT:-}" ]]; then

    prepareScript=".#nixosConfigurations.$1.config.wat.build.install-prepare"
    if [[ "$(nix eval "$prepareScript" --apply isNull)" == false ]]; then
      echo do prepare now
      ${nix-remote-run} -t "root@$1" "$prepareScript"
    fi

  fi

  system_installable=".#nixosConfigurations.$1.config.system.build.toplevel"

  echo Evaluate target system configuration
  ${nix-with-flakes} path-info --json "$system_installable" | ${jq}/bin/jq --raw-output ".[0].path" | read nixos_config_path

  echo Copy target system
  ${nix-with-flakes} copy --substitute-on-destination --to "ssh://root@$1?remote-store=/mnt" $system_installable

  system="$(ssh "root@$1" -- uname -m)-linux"
  ${nix-remote-run} "root@$1" "${wat}#packages.$system.wat-install-activation" "$nixos_config_path"
''
