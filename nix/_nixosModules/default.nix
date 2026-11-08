inputs:

{

  wat-installer = import ./installer.nix inputs;
  wat-machine-files = import ./machine-files.nix inputs;

  wat-installer-lib = import ./installer-lib.nix;

  wat-installer-btrfs = import ./installers/btrfs.nix;
  wat-installer-btrfs-luks = import ./installers/btrfs-luks.nix;
  wat-installer-hcloud = import ./installers/hcloud.nix;

}
