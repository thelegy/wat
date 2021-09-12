{ writeScript, coreutils, zsh, nix, nixos-enter }:

writeScript "activationScript" ''
  #!${zsh}/bin/zsh
  set -e -u -o pipefail

  # reset path to ensure the independence of this script
  export PATH=

  echo Setting system profile
  ${nix}/bin/nix-env --store /mnt --profile /mnt/nix/var/nix/profiles/system --set "$1"

  echo Creating /etc/NIXOS
  ${coreutils}/bin/mkdir -m 0755 -p "/mnt/etc"
  ${coreutils}/bin/touch /mnt/etc/NIXOS

  echo
  echo Linking mtab for grub
  ${coreutils}/bin/ln -sfn /proc/mounts /mnt/etc/mtab

  echo Installing bootloader
  ${nixos-enter}/bin/nixos-enter --root /mnt -c "NIXOS_INSTALL_BOOTLOADER=1 '$1/bin/switch-to-configuration' boot"

  echo Copy ssh keys from installer if none are present on the target system yet
  ${coreutils}/bin/cp --no-clobber /etc/ssh/ssh_host_* /mnt/etc/ssh

  ${coreutils}/bin/sync
''
