{ inputs, lib, ... }:
{
  flake-file.inputs.nix-unit = {
    url = "github:nix-community/nix-unit";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  imports = [ (inputs.nix-unit.modules.flake.default or { }) ];

  perSystem =
    { inputs', ... }:
    {
      nix-unit = {
        inputs = lib.removeAttrs inputs [ "self" ] // {
          # The root flake can be imported from a checkout. Do not carry its
          # Git metadata into nix-unit's sandboxed input override.
          wat = lib.cleanSource inputs.wat.outPath;
        };
        package = inputs'.nixpkgs.legacyPackages.nix-unit;
      };
    };
}
