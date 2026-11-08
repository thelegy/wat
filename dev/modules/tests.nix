{ ... }:
{
  perSystem =
    { pkgs, ... }:
    {
      checks.module-helpers =
        assert import ../../tests/module-helpers.nix { inherit (pkgs) lib; };
        pkgs.runCommand "wat-module-helpers" { } ''
          touch "$out"
        '';
    };
}
