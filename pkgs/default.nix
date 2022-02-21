{ self, nixpkgs, ...}:
final: prev:
with final.lib;
with final;

let
  wat = self;
in {

  wat-deploy = callPackage ./deploy.nix {};
  wat-gather = callPackage ./wat-gather.nix { inherit wat; };
  wat-gather-script = callPackage ./gatherScript.nix {};
  wat-install = callPackage ./wat-install.nix { inherit nixpkgs wat; };
  wat-install-activation = callPackage ./wat-install-activation.nix {};
  wat-install-helpers = callPackage ./wat-install-helpers {};
  wat-update-envrc = callPackage ./update-envrc.nix {};

  nix-remote-run = callPackage ./nix-remote-run.nix {};
  nix-with-flakes = callPackage ./nix-with-flakes.nix {};
  nixos-enter = callPackage ./nixos-enter.nix { inherit nixpkgs; };
  nixos-generate-config = callPackage ./nixos-generate-config.nix { inherit nixpkgs; };

  wat-run-tests = tests:
  let
    testResults = runTests tests;
  in
    if length testResults > 0 then
      traceSeqN 10 testResults (throw "At least one tests did not match its expected outcome")
    else
      final.emptyDirectory;

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
