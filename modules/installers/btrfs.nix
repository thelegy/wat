{ lib
, wat-installer-lib
, pkgs
, config
, ... }: with lib;

let

  cfg = config.wat.installer.btrfs;
  hostname = config.networking.hostName;
  hostUuid = config.wat.installer.hostUuid;

  inherit (wat-installer-lib) uuidgen;

  isEfi = cfg.bootloader == "efi";
  isGrub = cfg.bootloader == "grub";

in {

  options = {

    wat.installer.btrfs = {
      enable = mkEnableOption "Enable btrfs installer";

      bootloader = mkOption {
        type = types.enum [ "efi" "grub" ];
        default = "efi";
      };

      installDisk = mkOption {
        type = types.str;
      };

      installDiskIsSSD = mkOption {
        type = types.bool;
        default = true;
      };

      efiId = mkOption {
        type = types.str;
        default = substring 0 8 (uuidgen { name = "efi"; });
      };

      swapSize = mkOption {
        type = types.str;
        default = "2GiB";
      };

      swapUuid = mkOption {
        type = types.str;
        default = uuidgen { name = "swap"; };
      };

      systemUuid = mkOption {
        type = types.str;
        default = uuidgen { name = "system"; };
      };

      systemLabel = mkOption {
        type = types.str;
        default = "system_${hostname}";
      };

    };

  };

  config = mkIf cfg.enable {

    boot.initrd.availableKernelModules = [ "btrfs" ];

    fileSystems."/" = {
      device = "/dev/disk/by-uuid/${cfg.systemUuid}";
      fsType = "btrfs";
      options = [
        "noatime"
        "subvol=/${hostname}"
      ] ++ optional cfg.installDiskIsSSD "discard=async";
    };

    fileSystems."/boot" = mkIf isEfi {
      device = let
        id = toUpper ("${substring 0 4 cfg.efiId}-${substring 4 4 cfg.efiId}");
      in "/dev/disk/by-uuid/${id}";
      fsType = "vfat";
    };

    swapDevices = [{
      device = "/dev/disk/by-uuid/${cfg.swapUuid}";
    }];

    boot.loader.grub = mkIf isGrub {
      configurationLimit = 3;
      device = cfg.installDisk;
    };

    wat.build.installer = {

      format.configure = mkMerge (
        singleton ''
          installDisk=${escapeShellArg cfg.installDisk}
          swapUuid=${escapeShellArg cfg.swapUuid}
          systemUuid=${escapeShellArg cfg.systemUuid}
          systemLabel=${escapeShellArg cfg.systemLabel}
          hostname=${escapeShellArg hostname}

          partitionTable=()
        '' ++ optional isEfi ''
          efiId=${escapeShellArg cfg.efiId}

          partitionTable+='start=2048, size=512MiB, type=uefi, name="esp"'
          espPartition=''${installDisk:A}$#partitionTable
        '' ++ optional isGrub ''
          # Bios boot partition
          partitionTable+="size=1MiB, type=21686148-6449-6E6F-744E-656564454649"
        '' ++ singleton ''
          partitionTable+="size="${escapeShellArg cfg.swapSize}', type=swap, name="swap"'
          swapPartition=''${installDisk:A}$#partitionTable
        ''
      );

      format.wipe = mkMerge (
        singleton ''
          echo Wiping partition table
          ${pkgs.coreutils}/bin/dd if=/dev/zero of=$installDisk bs=1M count=1 conv=fsync

          echo Ensure partition table changes are known to the kernel
          ${pkgs.busybox}/bin/partprobe $installDisk
        '' ++ optional cfg.installDiskIsSSD ''
          echo Discard disk contents
          ${pkgs.util-linux}/bin/blkdiscard $installDisk
        ''
      );

      format.partitionOuter = mkMerge (
        singleton ''
          echo Creating partition table
          ${pkgs.util-linux}/bin/sfdisk $installDisk <<EOF
            label: gpt
            ''${(pj:\n  :)partitionTable}
            type=linux, name="system"
          EOF
          systemPartition=''${installDisk:A}$(( 1 + $#partitionTable ))

          echo Ensure partition table changes are known to the kernel
          ${pkgs.busybox}/bin/partprobe $installDisk
        '' ++ optional isEfi ''
          echo Create EFI partition
          ${pkgs.dosfstools}/bin/mkfs.fat -F32 -i $efiId -n ESP $espPartition
        ''
      );

      format.encryptionSetup = "";

      format.partitionInner = mkMerge (
        singleton ''
          echo Setup swap
          ${pkgs.util-linux}/bin/mkswap --label swap --uuid $swapUuid $swapPartition
          ${pkgs.util-linux}/bin/swapon $swapPartition

          echo Setup system partition
          ${pkgs.btrfsProgs}/bin/mkfs.btrfs --label $systemLabel --uuid $systemUuid $systemPartition
          ${pkgs.coreutils}/bin/mkdir -p /mnt
          mountOpts=(noatime)
        '' ++ optional cfg.installDiskIsSSD ''
          mountOpts+=(discard=async)
        '' ++ singleton ''
          ${pkgs.util-linux}/bin/mount -o ''${(j:,:)mountOpts} $systemPartition /mnt
          ${pkgs.btrfsProgs}/bin/btrfs subvolume create /mnt/$hostname
          ${pkgs.btrfsProgs}/bin/btrfs subvolume create /mnt/$hostname/nix
          ${pkgs.util-linux}/bin/umount /mnt

          #echo Remount the system
          mountOpts+="subvol=/$hostname"
          ${pkgs.util-linux}/bin/mount -o ''${(j:,:)mountOpts} $systemPartition /mnt
        '' ++ optional isEfi ''
          echo Mount the efi partition
          ${pkgs.coreutils}/bin/mkdir -p /mnt/boot
          ${pkgs.util-linux}/bin/mount $espPartition /mnt/boot
        ''
      );

    };

  };

}
