{ lib
, pkgs
, ... }: with lib;

{

  options.wat-installer-lib = mkOption {
    type = types.attrs;
    internal = true;
    default = {

      uuidgen = { namespace ? "59d93334-df87-4242-ac91-9c48886b4d94", name }:
        readFile (pkgs.runCommandNoCC "uuidgenerator" {} ''
          uuid=$(${pkgs.util-linux}/bin/uuidgen --sha1 --namespace ${escapeShellArg namespace} --name ${escapeShellArg name})
          echo -n "$uuid" > $out
        '');

    };
  };

}
