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

      efiPartUuid = mkOption {
        type = types.str;
        default = uuidgen { name = "efi-part"; };
      };

      swapSize = mkOption {
        type = types.str;
        default = "2GiB";
      };

      swapPartUuid = mkOption {
        type = types.str;
        default = uuidgen { name = "swap-part"; };
      };

      swapUuid = mkOption {
        type = types.str;
        default = uuidgen { name = "swap"; };
      };

      systemPartUuid = mkOption {
        type = types.str;
        default = uuidgen { name = "system-part"; };
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
      device = "/dev/disk/by-partuuid/${cfg.efiPartUuid}";
      fsType = "vfat";
    };

    swapDevices = [{
      device = "/dev/disk/by-uuid/${cfg.swapUuid}";
    }];

    boot.loader.grub = mkIf isGrub {
      configurationLimit = 3;
      device = cfg.installDisk;
    };

    boot.loader.systemd-boot.enable = mkIf isEfi true;

    wat.build.installer = {

      format.fragments = {

        configure = {
          content = mkMerge (
            singleton ''
              installDisk=${escapeShellArg cfg.installDisk}
              swapPartUuid=${escapeShellArg cfg.swapPartUuid}
              swapUuid=${escapeShellArg cfg.swapUuid}
              systemPartUuid=${escapeShellArg cfg.systemPartUuid}
              systemUuid=${escapeShellArg cfg.systemUuid}
              systemLabel=${escapeShellArg cfg.systemLabel}
              hostname=${escapeShellArg hostname}
              swapSize=${escapeShellArg cfg.swapSize}

              typeset -A partitionTable
              partitionTable=([99system]="type=linux, name=\"system\", uuid=\"''${(q)systemPartUuid}\"")
            '' ++ optional isEfi ''
              efiPartUuid=${escapeShellArg cfg.efiPartUuid}

              partitionTable[0esp]="start=2048, size=512MiB, type=uefi, name=\"esp\", uuid=\"''${(q)efiPartUuid}\""
            '' ++ optional isGrub ''
              partitionTable[0bios]="size=1MiB, type=21686148-6449-6E6F-744E-656564454649"
            '' ++ singleton ''
              partitionTable[10swap]="size=$swapSize, type=swap, name=\"swap\", uuid=\"''${(q)swapPartUuid}\""
            ''
          );
        };

        wipe = {
          after = [ "configure" ];
          content = mkMerge (
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
        };

        partitionOuter = {
          after = [ "wipe" ];
          content = mkMerge (
            singleton ''
              echo Creating partition table
              typeset -a partitionTableProcessed
              for k in ''${(kn)partitionTable}; partitionTableProcessed+=($partitionTable[$k])
              ${pkgs.util-linux}/bin/sfdisk $installDisk <<EOF
                label: gpt
                ''${(pj:\n  :)partitionTableProcessed}
              EOF

              echo Ensure partition table changes are known to the kernel
              ${pkgs.busybox}/bin/partprobe $installDisk
              ${pkgs.udev}/bin/udevadm settle
            '' ++ optional isEfi ''
              echo Create EFI partition
              : ''${espPartition:=/dev/disk/by-partuuid/$efiPartUuid}
              ${pkgs.dosfstools}/bin/mkfs.fat -F32 -n ESP $espPartition
            ''
          );
        };

        partitionInner = {
          after = [ "partitionOuter" ];
          content = mkMerge (
            singleton ''
              echo Setup swap
              : ''${swapPartition:=/dev/disk/by-partuuid/$swapPartUuid}
              ${pkgs.util-linux}/bin/mkswap --label swap --uuid $swapUuid $swapPartition
              ${pkgs.util-linux}/bin/swapon $swapPartition

              echo Setup system partition
              : ''${systemPartition:=/dev/disk/by-partuuid/$systemPartUuid}
              ${pkgs.btrfs-progs}/bin/mkfs.btrfs --label $systemLabel --uuid $systemUuid $systemPartition
              ${pkgs.coreutils}/bin/mkdir -p /mnt
              mountOpts=(noatime)
            '' ++ optional cfg.installDiskIsSSD ''
              mountOpts+=(discard=async)
            '' ++ singleton ''
              ${pkgs.util-linux}/bin/mount -o ''${(j:,:)mountOpts} $systemPartition /mnt
              ${pkgs.btrfs-progs}/bin/btrfs subvolume create /mnt/$hostname
              ${pkgs.btrfs-progs}/bin/btrfs subvolume create /mnt/$hostname/nix
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


    };
  };

}
