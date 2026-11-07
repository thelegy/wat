{
  inputs,
  ...
}:

{

  config.perSystem =
    { system, inputs', ... }:
    let
      pkgs = import inputs.nixpkgs {
        inherit system;
        overlays = [ inputs.wat.overlays.default ];
      };
    in
    {

      packages = {
        inherit (pkgs) wat-deploy-tools;
      };

      devShells.default = pkgs.mkShellNoCC {
        name = "wat";
        packages = [
          pkgs.wat-deploy-tools
          inputs'.wat.packages.default
        ];
      };

    };

}
