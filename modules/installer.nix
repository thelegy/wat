flakes:
{ lib, config, pkgs, ... }:
with lib;

let

  cfg = config.wat;
  hostname = config.networking.hostName;

in {

  options = {

    wat.installer.enable = mkEnableOption "";

    wat.installer.installDisk = mkOption {
      type = types.str;
    };

    wat.installer.efiId = mkOption {
      type = types.str;
    };

    wat.installer.luksUuid = mkOption {
      type = types.str;
    };

    wat.installer.swapUuid = mkOption {
      type = types.str;
    };

    wat.build.install-prepare = mkOption {
      type = with types; nullOr package;
      internal = true;
      default = null;
    };

  };

  config = mkIf cfg.installer.enable {

    wat.build.install-prepare = pkgs.wat-install-helpers.format-luks-lvm-bcachefs {
      nixosConfig = config;
      installDisk = cfg.installer.installDisk;
      efiId = cfg.installer.efiId;
      luksUuid = cfg.installer.luksUuid;
      swapUuid = cfg.installer.swapUuid;
    };

    boot.kernelPackages = pkgs.linuxPackages_testing_bcachefs;

    boot.initrd.availableKernelModules = [ "bcache" ];

    boot.initrd.luks.devices."${hostname}" = {
      device = "/dev/disk/by-uuid/${cfg.installer.luksUuid}";
      allowDiscards = true;
    };

    fileSystems."/" = {
      device = "/dev/mapper/vg_${hostname}-system";
      fsType = "bcachefs";
    };

    fileSystems."/boot" = {
      device = "/dev/disk/by-uuid/${cfg.installer.efiId}";
      fsType = "vfat";
    };

    swapDevices = [{
      device = "/dev/mapper/vg_${hostname}-swap";
    }];

  };

}
