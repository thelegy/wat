flakes@{ self, ... }:
with self.lib;

{

  wat-installer = import ./installer.nix flakes;

}
