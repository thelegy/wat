{ writeScript
, lib

, bcachefs-tools
, busybox
, coreutils
, cryptsetup
, dosfstools
, jq
, lvm2
, util-linux
, zsh

, installDisk
, nixosConfig
, efiId
, luksUuid
, swapSize ? "4G"
, swapUuid
}:
with lib;

let

  hostName = nixosConfig.networking.hostName;
  espPartition = ''$(${coreutils}/bin/realpath "${installDisk}")1'';
  luksPartition = ''$(${coreutils}/bin/realpath "${installDisk}")2'';
  luksVolName = "cryptvol_${hostName}";
  lvmPartition = "/dev/mapper/${luksVolName}";
  vgName = "vg_${hostName}";
  swapPartition = "/dev/${vgName}/swap";
  systemPartition = "/dev/${vgName}/system";

in writeScript "format-luks-lvm-bcachefs" ''
  #!${zsh}/bin/zsh
  set -e -u -o pipefail

  # reset path to ensure the independence of this script
  export PATH=

  preData="$(</dev/stdin)"
  luksPassphrase="$(${jq}/bin/jq -r '.luksPassphrase' <<<$preData)"
  unset preData

  echo Install now

  echo Wiping partition table
  ${coreutils}/bin/dd if=/dev/zero "of=${installDisk}" bs=1M count=1 conv=fsync

  echo Ensure partition table changes are known to the kernel
  ${busybox}/bin/partprobe "${installDisk}"

  echo Discard disk contents
  ssd=false
  if ${util-linux}/bin/blkdiscard "${installDisk}"; then
    ssd=true
  fi

  echo Creating partition table
  ${util-linux}/bin/sfdisk "${installDisk}" <<EOF
    label: gpt
    start=2048, size=512MiB, type=C12A7328-F81F-11D2-BA4B-00A0C93EC93B, name="esp"
    type=0FC63DAF-8483-4772-8E79-3D69D8477DE4, name="system"
  EOF

  echo Ensure partition table changes are known to the kernel
  ${busybox}/bin/partprobe "${installDisk}"

  echo Create EFI partition
  ${dosfstools}/bin/mkfs.fat -F32 -i "${replaceStrings ["-"] [""] efiId}" -n ESP "${espPartition}"

  echo Create LUKS cryptvol
  ${cryptsetup}/bin/cryptsetup --batch-mode --key-file <(echo -n "$luksPassphrase") luksFormat --type luks2 --uuid "${luksUuid}" "${luksPartition}"

  echo Mounting LUKS cryptvol for the first time
  luksSsdOptions=""
  if $ssd; then
    luksSsdOptions=(--allow-discards --persistent)
  fi
  ${cryptsetup}/bin/cryptsetup --batch-mode --key-file <(echo -n "$luksPassphrase") $luksSsdOptions open "${luksPartition}" "${luksVolName}"
  unset luksPassphrase

  echo Setup lvm
  ${lvm2.bin}/bin/pvcreate "${lvmPartition}"
  ${lvm2.bin}/bin/vgcreate "${vgName}" "${lvmPartition}"

  ${lvm2.bin}/bin/lvcreate --size "${swapSize}" --name swap --yes "${vgName}"
  ${lvm2.bin}/bin/lvcreate --extents "100%FREE" --name system --yes "${vgName}"

  echo Setup Swap
  ${util-linux}/bin/mkswap --label swap --uuid "${swapUuid}" "${swapPartition}"
  ${util-linux}/bin/swapon "${swapPartition}"

  echo Create system filesystem
  ${bcachefs-tools}/bin/bcachefs format --discard "${systemPartition}"

  echo Mount the system
  ${coreutils}/bin/mkdir -p /mnt
  ${util-linux}/bin/mount -t bcachefs "${systemPartition}" /mnt

  ${coreutils}/bin/mkdir -p /mnt/boot
  ${util-linux}/bin/mount "${espPartition}" /mnt/boot
''
