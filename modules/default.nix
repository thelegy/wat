flakes@{ self, ... }:
with self.lib;

{

  wat-installer = import ./installer.nix flakes;

  wat-installer-lib = import ./installer-lib.nix;

  wat-installer-btrfs = import ./installers/btrfs.nix;
  wat-installer-hcloud = import ./installers/hcloud.nix;

}
