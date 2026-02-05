rec {

  withPkgsFor =
    systems: nixpkgs: overlays: fn:
    with nixpkgs.lib;
    genAttrs systems (system: fn (import nixpkgs { inherit system overlays; }));

  withPkgsForLinux = nixpkgs: withPkgsFor nixpkgs.lib.platforms.linux nixpkgs;

  mkMachine = import ./mkMachine.nix;

  wrapModules = import ./mkModule.nix;

}
