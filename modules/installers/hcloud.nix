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
    (import "${modulesPath}/profiles/qemu-guest.nix" { inherit config lib; })
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
          dns = mkDefault [
            "2a01:4ff:ff00::add:1"
            "2a01:4ff:ff00::add:2"
            "185.12.64.1"
            "185.12.64.2"
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

      networking.timeServers = mkDefault [
        "ntp1.hetzner.de"
        "ntp2.hetzner.com"
        "ntp3.hetzner.net"
      ];

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
