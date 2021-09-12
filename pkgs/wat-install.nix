{ writeScriptBin

, nixpkgs
, wat

, jq
, nix-remote-run
, nix-with-flakes
, util-linux
, zsh
}:

writeScriptBin "wat-install" ''
  #!${zsh}/bin/zsh
  set -e -u -o pipefail

  system="$(ssh "root@$1" -- uname -m)-linux"

  if [[ -n "''${WAT_DO_FORMAT:-}" ]]; then
    formatScript=".#nixosConfigurations.$1.config.wat.build.install-format"
    installDisk="$(${nix-with-flakes} eval --raw ".#nixosConfigurations.$1.config.wat.installer.installDisk")"
    if [[ "$(${nix-with-flakes} eval "$formatScript" --apply isNull)" == false ]]; then

      ${nix-remote-run} -b lsblk "root@$1" "${nixpkgs}#legacyPackages.$system.util-linux" "$installDisk"

      echo "!!! This will WIPE THE WHOLE DISK \"$installDisk\". Please type the Hostname \"$1\" to verify this and continue:" >&2
      read -r confirmation
      if [[ "$confirmation" != "$1" ]]; then
        echo Aborting the installation. >&2
        exit 1
      fi

      echo Please enter the luks passphrase... >&2
      read -rs luksPassphrase

      echo Please reenter the luks passphrase to confirm... >&2
      read -rs luksPassphraseConfirm

      if [[ "$luksPassphrase" != "$luksPassphraseConfirm" ]]; then
        echo Passphrases did not match >&2
        exit 1
      fi

      preResult="
        {
          \"luksPassphrase\": $(${jq}/bin/jq -R <<<$luksPassphrase)
        }
      "

      echo "$preResult" | ${nix-remote-run} "root@$1" "$formatScript"

      unset preResult
    fi

  fi

  system_installable=".#nixosConfigurations.$1.config.system.build.toplevel"

  echo Evaluate target system configuration
  ${nix-with-flakes} path-info --json "$system_installable" | ${jq}/bin/jq --raw-output ".[0].path" | read nixos_config_path

  echo Copy target system
  ${nix-with-flakes} copy --substitute-on-destination --to "ssh://root@$1?remote-store=/mnt" $system_installable

  ${nix-remote-run} "root@$1" "${wat}#packages.$system.wat-install-activation" "$nixos_config_path"
''
