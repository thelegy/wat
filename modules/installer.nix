flakes:
{ lib, config, pkgs, ... }:
with lib;

let

  cfg = config.wat.build;
  hostname = config.networking.hostName;

in {

  options = {

    wat.build.installer = {
      format.configure = mkOption {
        type = types.lines;
        internal = true;
      };

      format.wipe = mkOption {
        type = types.lines;
        internal = true;
      };

      format.partiionOuter = mkOption {
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

      ${cfg.installer.format.partiionOuter}

      ${cfg.installer.format.encryptionSetup}

      ${cfg.installer.format.partitionInner}
    '';

  };

}
