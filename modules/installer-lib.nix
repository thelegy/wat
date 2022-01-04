{ lib
, config
, pkgs
, ... }: with lib;

{

  _module.args.wat-installer-lib = {

    uuidgen = { namespace ? config.wat.installer.hostUuid, name }:
      readFile (pkgs.runCommandNoCC "uuidgenerator" {} ''
        uuid=$(${pkgs.util-linux}/bin/uuidgen --sha1 --namespace ${escapeShellArg namespace} --name ${escapeShellArg name})
        echo -n "$uuid" > $out
      '');

  };

}
