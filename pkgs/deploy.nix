{ lib, writeScriptBin }:
with lib;

writeScriptBin "deploy" ''
  #!/usr/bin/env zsh

  set -e
  # fail on undeclared variable
  set -u
  set -o pipefail

  readonly cmdname=$(basename $0)

  # Allow running the scripts from the bin directory instead of the repository root
  if [[ $(basename $(realpath .)) = bin ]]
  then
    cd ..
  fi

  # This script cannot run without the nixos configuration entry point
  if [[ ! -f flake.nix ]]
  then
    print -P "%B%F{red}Error: %F{orange}./flake.nix%F{red} not found in current directory%b%f" >&2
    exit 2
  fi


  source ${./util.zsh}

  usage() {
    print "Usage: $cmdname <hostname> [switch|boot|reboot|test|dry-activate|build|iso|sdcard]" >&2
  }

  positional=()
  while [[ $# -gt 0 ]]
  do
    case "$1" in
      --help|-h)
        usage
        exit 0
        ;;
      *)
        positional+=("$1")
        ;;
    esac
    shift
  done

  if [[ ''${#positional[@]} -ne 1 && ''${#positional[@]} -ne 2 ]]
  then
    print "Invalid number of arguments." >&2
    usage
    exit 2
  fi

  readonly hostname="''${positional[1]}"
  if [[ ''${#positional[@]} -ge 2 ]]
  then
    readonly original_operation="''${positional[2]}"
  else
    # default operation
    readonly original_operation=switch
  fi

  readonly nix_options=(--log-format bar-with-logs)
  operation=$original_operation
  set_profile=""
  reboot=""

  if [[ "$operation" = "switch" || "$operation" = "boot" ]]
  then
    set_profile=1
  elif [[ "$operation" = "reboot" ]]
  then
    operation="boot"
    set_profile=1
    reboot=1
  elif [[ "$operation" = "test" || "$operation" = "dry-activate" || "$operation" = "build" || "$operation" = "iso" || "$operation" = "sdcard" ]]
  then
    # pass
  else
    print_error "Invalid operation: $operation"
    usage
    exit 2
  fi

  if [[ "$(hostname)" = "$hostname" ]]
  then
    readonly is_target_host=1
  else
    readonly is_target_host=""
  fi


  readonly local_temp_dir=$(mktemp --tmpdir --directory phoenix-deploy.XXXXXXXXXX)
  trap "rm -rf $local_temp_dir" EXIT INT HUP TERM

  if [[ "$operation" == "iso" ]]
  then
    print_info "Building iso image"
    nix $nix_options build .#nixosConfigurations.$hostname.config.system.build.iso --out-link "$local_temp_dir/nixos-iso-$hostname"
    readonly nixos_iso_path=$(realpath "$local_temp_dir/nixos-iso-$hostname")
    print $nixos_iso_path
    exit 0
  fi

  if [[ "$operation" == "sdcard" ]]
  then
    print_info "Building sdcard image"
    nix $nix_options build .#nixosConfigurations.$hostname.config.system.build.sdImage --out-link "$local_temp_dir/nixos-sdcard-$hostname"
    readonly nixos_sdcard_path=$(realpath "$local_temp_dir/nixos-sdcard-$hostname")
    print $nixos_sdcard_path
    exit 0
  fi

  print_info "Building target system configuration"
  nix $nix_options build --keep-going .#nixosConfigurations.$hostname.config.system.build.toplevel --out-link "$local_temp_dir/nixos-config-$hostname"
  readonly nixos_config_path=$(realpath "$local_temp_dir/nixos-config-$hostname")

  if [[ "$operation" = "build" ]]
  then
    print_info "Build completed"
    print $nixos_config_path
    exit 0
  fi

  print_info "Deploying target system configuration"
  if [[ "$is_target_host" ]]
  then
    # local deploy

    if [[ -n "$set_profile" ]]
    then
      sudo nix-env --profile /nix/var/nix/profiles/system --set $nixos_config_path
    fi
    sudo $nixos_config_path/bin/switch-to-configuration $operation
    sync

    if [[ -n "$reboot" ]]
    then
      sudo systemctl reboot
    fi
  else
    # remote deploy

    nix $nix_options copy --substitute-on-destination --to "ssh://root@$hostname" .#nixosConfigurations.$hostname.config.system.build.toplevel

    # The manual way to do it (this is in theory also supported by nixos-rebuild by using '-I')

    if [[ -n "$set_profile" ]]
    then
      ssh root@$hostname "nix-env --profile /nix/var/nix/profiles/system --set $nixos_config_path"
    fi
    ssh root@$hostname "$nixos_config_path/bin/switch-to-configuration $operation && sync"

    if [[ -n "$reboot" ]]
    then
      ssh root@$hostname "systemctl reboot"
    fi
  fi


  print_info "Update completed"
''
