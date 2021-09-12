{ self, nixpkgs, ...}:
final: prev:
with final.lib;
with final;

{

  wat-deploy = callPackage ./deploy.nix {};
  wat-gather = callPackage ./wat-gather.nix { wat = self; };
  wat-gather-script = callPackage ./gatherScript.nix {};
  wat-install = callPackage ./wat-install.nix { wat = self; };
  wat-install-activation = callPackage ./wat-install-activation.nix {};
  wat-install-helpers = callPackage ./wat-install-helpers {};
  wat-update-envrc = callPackage ./update-envrc.nix {};

  nix-remote-run = callPackage ./nix-remote-run.nix {};
  nix-with-flakes = callPackage ./nix-with-flakes.nix {};
  nixos-enter = callPackage ./nixos-enter.nix { inherit nixpkgs; };
  nixos-generate-config = callPackage ./nixos-generate-config.nix { inherit nixpkgs; };

  wat-deploy-tools = symlinkJoin {
    name = "wat-deploy-tools";
    paths = [
      wat-deploy
      wat-gather
      wat-install
      wat-update-envrc
    ];
  };

  wat-deploy-tools-envrc = writeText "wat-deploy-tools-envrc" ''
    PATH_add ${wat-deploy-tools}/bin
  '';

}
