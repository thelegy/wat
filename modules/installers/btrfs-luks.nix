{ lib
, wat-installer-lib
, pkgs
, config
, ... }: with lib;


let
  cfg = config.wat.installer.btrfs.luks;
  isGrub = config.wat.installer.btrfs.bootloader == "grub";
  inherit (wat-installer-lib) uuidgen;
in
{

  options = {

    wat.installer.btrfs.luks = {

      enable = mkEnableOption "Enable luks disk encryption";

      luksUuid = mkOption {
        type = types.str;
        default = uuidgen { name = "system-luks"; };
      };

      bootId = mkOption {
        type = types.str;
        default = substring 0 8 (uuidgen { name = "boot"; });
      };

    };

  };

  config = mkIf cfg.enable {

    boot.initrd.luks.devices."${config.networking.hostName}" = {
      device = "/dev/disk/by-uuid/${cfg.luksUuid}";
      allowDiscards = true;
    };

    fileSystems."/boot" = mkIf isGrub {
      device = let
        id = toUpper ("${substring 0 4 cfg.bootId}-${substring 4 4 cfg.bootId}");
      in "/dev/disk/by-uuid/${id}";
      fsType = "vfat";
    };

    wat.build.installer.launcher.fragments = {
      formatConfig = {
        before = [ "format" ];
        content = localPkgs: ''
          echo Please enter the luks passphrase... >&2
          read -rs luksPassphrase

          echo Please reenter the luks passphrase to confirm... >&2
          read -rs luksPassphraseConfirm

          if [[ $luksPassphrase != $luksPassphraseConfirm ]]; then
            echo Passphrases did not match >&2
            exit 1
          fi

          formatConfig="
            {
              \"luksPassphrase\": $(${localPkgs.jq}/bin/jq -R <<<$luksPassphrase)
            }
          "
        '';
      };
    };

    wat.build.installer.format.fragments = {

      luksConfigure = {
        after = [ "configure" ];
        before = [ "wipe" ];
        content = mkMerge (
          singleton ''
            luksUuid=${escapeShellArg cfg.luksUuid}
            preData=$(</dev/stdin)
            luksPassphrase=$(${pkgs.jq}/bin/jq -r '.luksPassphrase' <<<$preData)
            unset preData

            luksVolName=cryptvol_$hostname
            lvmPartition=/dev/mapper/$luksVolName
            vgName=vg_$hostname
            swapPartition=/dev/$vgName/swap
            systemPartition=/dev/$vgName/system
          '' ++ optional isGrub ''
            partitionTable[5boot]='size=512MiB, type=linux, name="boot"'
            bootId=${escapeShellArg cfg.bootId}
          ''
        );
      };

      luksSetup = {
        after = [ "partitionOuter" ];
        before = [ "partitionInner" ];
        content = mkMerge (
          optional isGrub ''
              echo Create boot partition
              : ''${bootPartition:=''${installDisk:A}$(partitionId 5boot)}
              ${pkgs.dosfstools}/bin/mkfs.fat -F32 -i $bootId -n BOOT $bootPartition
          '' ++ singleton ''
            echo Create LUKS cryptvol
            : ''${luksPartition:=''${installDisk:A}$(partitionId 99system)}
            ${pkgs.cryptsetup}/bin/cryptsetup --batch-mode --key-file <(echo -n $luksPassphrase) luksFormat --type luks2 --uuid $luksUuid $luksPartition

            echo Mounting LUKS cryptvol for the first time
            luksSsdOptions=()
          '' ++ optional config.wat.installer.btrfs.installDiskIsSSD ''
            luksSsdOptions=(--allow-discards --persistent)
          '' ++ singleton ''
            ${pkgs.cryptsetup}/bin/cryptsetup --batch-mode --key-file <(echo -n $luksPassphrase) $luksSsdOptions open $luksPartition $luksVolName
            unset luksPassphrase

            echo Setup lvm
            ${pkgs.lvm2.bin}/bin/pvcreate $lvmPartition
            ${pkgs.lvm2.bin}/bin/vgcreate $vgName $lvmPartition

            ${pkgs.lvm2.bin}/bin/lvcreate --size $swapSize --name swap --yes $vgName
            ${pkgs.lvm2.bin}/bin/lvcreate --extents "100%FREE" --name system --yes $vgName
          ''
        );
      };

      mountBoot = {
        after = [ "partitionInner" ];
        content = mkIf isGrub ''
          echo Mount the boot partition
          ${pkgs.coreutils}/bin/mkdir -p /mnt/boot
          ${pkgs.util-linux}/bin/mount $bootPartition /mnt/boot
        '';
      };

    };

  };

}
