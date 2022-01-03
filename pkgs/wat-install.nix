{ writeScriptBin

, nixpkgs
, wat

, git
, jq
, nix-remote-run
, nix-with-flakes
, openssh
, util-linux
, zsh
}:

writeScriptBin "wat-install" ''
  #!${zsh}/bin/zsh
  set -euo pipefail

  PATH=
  path+="${git}/bin"
  path+="${openssh}/bin"

  usage() {
    echo "$0 MACHINE [--[no-]format]"
    echo "     --[no-]format to skip the format step"
    echo "  -y --yes to answer all questions with yes. Use with care!"
    echo "  -h --help show this error message"
    echo
  }

  zparseopts -D -E -A opts h -help -{no-,}format y -yes

  if [[ -v opts[-h] || -v opts[--help] ]] {
    usage
    exit 0
  }

  : ''${WAT_DO_FORMAT:=1}
  if [[ -v opts[--format] ]] { WAT_DO_FORMAT=1 }
  if [[ -v opts[--no-format] ]] { WAT_DO_FORMAT=0 }

  : ''${WAT_ALWAYS_YES:=0}
  if [[ -v opts[-y] || -v opts[--yes] ]] { WAT_ALWAYS_YES=1 }

  machine=$1
  if [[ $#argv != 1 ]] {
    usage
    exit 1
  }
  system=$(ssh root@$machine -- uname -m)-linux

  if (( $WAT_DO_FORMAT )) {
    formatScript=".#nixosConfigurations.$machine.config.wat.build.installer.format.script"
    if [[ $(${nix-with-flakes} eval $formatScript --apply isNull) == false ]] {
      if (( ! $WAT_ALWAYS_YES )) {
        ${nix-remote-run} -b lsblk root@$machine "${nixpkgs}#legacyPackages.$system.util-linux"
        echo "!!! This will WIPE THE WHOLE DISK. Please type the Hostname \"$machine\" to verify this and continue:" >&2
        read -r confirmation
        if [[ $confirmation != $machine ]] {
          echo Aborting the installation. >&2
          exit 1
        }
      }

      ${nix-remote-run} root@$machine $formatScript
    }

  }

  system_installable=".#nixosConfigurations.$machine.config.system.build.toplevel"

  echo Evaluate target system configuration
  ${nix-with-flakes} path-info --json $system_installable | ${jq}/bin/jq --raw-output ".[0].path" | read nixos_config_path

  echo Copy target system
  ${nix-with-flakes} copy --substitute-on-destination --to "ssh://root@$machine?remote-store=/mnt" $system_installable

  ${nix-remote-run} root@$machine "${wat}#packages.$system.wat-install-activation" $nixos_config_path
''
