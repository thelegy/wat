flakes:
{ lib
, wat-installer-lib
, config
, pkgs
, ... }: with lib;

let

  cfg = config.wat.build;
  hostname = config.networking.hostName;

  inherit (wat-installer-lib) uuidgen;

in {

  options = {

    wat.installer.repoUuid = mkOption {
      type = types.str;
    };

    wat.installer.hostUuid = mkOption {
      type = types.str;
      default = uuidgen { namespace = config.wat.installer.repoUuid; name = hostname; };
    };

    wat.build.installer = {
      format.configure = mkOption {
        type = types.lines;
        internal = true;
      };

      format.wipe = mkOption {
        type = types.lines;
        internal = true;
      };

      format.partitionOuter = mkOption {
        type = types.lines;
        internal = true;
      };

      format.encryptionSetup = mkOption {
        type = types.lines;
        internal = true;
      };

      format.partitionInner = mkOption {
        type = types.lines;
        internal = true;
      };
    };

    wat.build.installer.format.script = mkOption {
      type = types.package;
      internal = true;
    };

  };

  config = {

    wat.build.installer.format.script = pkgs.writeScript "wat-installer-${hostname}" ''
      #!${pkgs.zsh}/bin/zsh
      set -euo pipefail

      # reset path to ensure the independence of this script
      export PATH=

      ${cfg.installer.format.configure}

      echo Install now

      ${cfg.installer.format.wipe}

      ${cfg.installer.format.partitionOuter}

      ${cfg.installer.format.encryptionSetup}

      ${cfg.installer.format.partitionInner}
    '';

  };

}
