flakes@{ self, ... }:
with self.lib;

{

  wat-installer = import ./installer.nix flakes;
  wat-machine-files = import ./machine-files.nix;

  wat-installer-lib = import ./installer-lib.nix;

  wat-installer-btrfs = import ./installers/btrfs.nix;
  wat-installer-btrfs-luks = import ./installers/btrfs-luks.nix;
  wat-installer-hcloud = import ./installers/hcloud.nix;

}
