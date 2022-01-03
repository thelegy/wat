{ lib
, config
, modulesPath
, ... }: with lib;

let
  cfg = config.wat.installer.hcloud;
in {

  options = {

    wat.installer.hcloud.enable = mkEnableOption "Enable hcloud installer";

    wat.installer.hcloud.macAddress = mkOption {
      type = types.str;
    };

    wat.installer.hcloud.ipv4Address = mkOption {
      type = with types; nullOr str;
      default = null;
    };

    wat.installer.hcloud.ipv6Address = mkOption {
      type = with types; nullOr str;
      default = null;
    };

  };

  config = mkIf cfg.enable (mkMerge [
    (import "${modulesPath}/profiles/qemu-guest.nix" {})
    {

      wat.installer.btrfs = {
        enable = true;
        bootloader = "grub";
        installDisk = "/dev/sda";
      };

      networking.useDHCP = false;
      systemd.network = {
        enable = true;
        networks.default = {
          matchConfig.MACAddress = cfg.macAddress;
          address = optional (!isNull cfg.ipv6Address) cfg.ipv6Address;
          dns = [
            "213.133.98.98"
            "213.133.99.99"
            "213.133.100.100"
          ];
          gateway = [
            "172.31.1.1"
            "fe80::1"
          ];
          addresses = optional (!isNull cfg.ipv4Address) {
            addressConfig = {
              Address = cfg.ipv4Address;
              Peer = "172.31.1.1";
            };
          };
        };
      };


      boot.initrd.availableKernelModules = [
        "ahci"
        "ata_piix"
        "sd_mod"
        "sr_mod"
        "uhci_hcd"
        "virtio_pci"
        "xhci_pci"
      ];
      boot.initrd.kernelModules = [
        "dm-snapshot"
      ];
      boot.kernelModules = [
      ];
      boot.extraModulePackages = [
      ];

    }
  ]);

}
