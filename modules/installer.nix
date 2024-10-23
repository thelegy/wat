flakes:
{ watLib
, wat-installer-lib
, config
, pkgs
, ...
}: with watLib;

let

  cfg = config.wat.build;
  hostname = config.networking.hostName;
  noInstallerError = throw "No installation method was defined for machine \"${hostname}\"!";

  inherit (wat-installer-lib) uuidgen;

in {

  options = {

    wat.installer.repoUuid = mkOption {
      type = types.str;
      internal = true;
    };

    wat.installer.hostUuid = mkOption {
      type = types.str;
      default = uuidgen { namespace = config.wat.installer.repoUuid; name = hostname; };
    };

    wat.build.installer = {

      activationScript = mkOption {
        type = types.package;
        internal = true;
      };

      format.fragments = mkOption {
        type = with types; dependencyDagOfSubmodule {
          options = {
            content = mkOption {
              type = lines;
            };
          };
        };
      };

      launcher.options = mkOption {
        type = with types; attrsOf (submodule {
          options = {
            aliases = mkOption {
              type = listOf str;
              internal = true;
              default = [];
            };
            argument = mkOption {
              type = bool;
              internal = true;
              default = false;
            };
            help = mkOption {
              type = str;
              internal = true;
              default = "";
            };
            enabled = mkOption {
              type = bool;
              internal = true;
              default = true;
            };
          };
        });
        internal = true;
      };

      launcher.fragments = mkOption {
        type = with types; dependencyDagOfSubmodule {
          options = {
            content = mkOption {
              type = functionTo lines;
            };
          };
        };
        internal = true;
      };

    };

    wat.build.installer.format.script = mkOption {
      type = types.package;
      internal = true;
    };

    wat.build.installer.launcher.script = mkOption {
      type = with types; attrsOf package;
      internal = true;
    };

  };

  config = let

    options = let
      origOptions = cfg.installer.launcher.options;
      optionNames = attrNames origOptions;
      allOptions = forEach optionNames (option: origOptions.${option} // { inherit option; });
    in filter (x: x.enabled) allOptions;

  in {

    wat.build.installer.activationScript = pkgs.wat-install-activation;

    wat.build.installer.format.fragments = {
      resetPath = {
        after = [ "veryEarly" ];
        before = [ "early" ];
        content = ''
          # reset path to ensure the independence of this script
          path=(${escapeShellArgs [
            "${pkgs.coreutils}/bin"
          ]})
        '';
      };
    };

    wat.build.installer.format.script = let
      fragments = types.dependencyDagOfSubmodule.toOrderedList cfg.installer.format.fragments;
    in pkgs.writeScript "wat-installer-${hostname}" ''
      #!${pkgs.zsh}/bin/zsh
      set -euo pipefail

      ${concatMapStringsSep "\n" (x: x.content) fragments}
    '';

    wat.build.installer.launcher.options = {
      "-help" = { aliases = ["h"]; help = "Display this help"; enabled = mkForce true; };
      "-foo" = { help = "Foos $PATH"; };
      "-bar" = { aliases = ["b"]; help = "Bars very hard"; };
      "-target" = { aliases = ["t"]; help = "SSH destination to install to"; argument = true; };
    };

    wat.build.installer.launcher.fragments = {
      resetPath = {
        after = [ "veryEarly" ];
        before = [ "early" ];
        content = localPkgs: ''
          # reset path to ensure the independence of this script
          path=(${escapeShellArgs [
            "${localPkgs.coreutils}/bin"
            "${localPkgs.git}/bin"
            "${localPkgs.openssh}/bin"
          ]})
        '';
      };
      usageFunction = {
        after = [ "resetPath" ];
        before = [ "early" ];
        content = localPkgs: ''
          usage() cat <<'EOF'
          ${concatMapStringsSep "\n" (opt:
            concatStringsSep " " (flatten [
              "  -${opt.option}:"
              (optional (length opt.aliases > 0) "(${concatMapStringsSep ", " (x: "-"+x) opt.aliases})")
              opt.help
            ])
          ) options}
          EOF
        '';
      };
      optionParser = {
        after = [ "resetPath" ];
        before = [ "early" ];
        content = localPkgs: ''
          zparseopts -F -M -A opts ${escapeShellArgs (flatten (forEach options (opt: let
            modifier = if opt.argument then ":" else "";
          in [ (opt.option + modifier) ] ++ forEach opt.aliases (alias: "${alias}${modifier}=${opt.option}")
          )))}
        '';
      };
      printHelp = {
        after = [ "usageFunction" "optionParser" ];
        before = [ "early" "options" ];
        content = localPkgs: ''
          if [[ -v opts[--help] ]] {
            usage
            exit 0
          }
        '';
      };
      targetOption = {
        after = [ "options" ];
        before = [ "early" ];
        content = localPkgs: ''
          : ''${WAT_TARGET:=root@$machine}
          if [[ -v opts[--target] ]] { WAT_TARGET=''${opts[--target]} }
        '';
      };
      format = {
        content = localPkgs: ''
          ${localPkgs.nix-remote-run} -b lsblk $WAT_TARGET "${pkgs.path}#legacyPackages.$system.util-linux.bin"
          echo "!!! This will WIPE THE WHOLE DISK. Please type the Hostname \"$machine\" to verify this and continue:" >&2
          read -r confirmation
          if [[ $confirmation != $machine ]] {
            echo Aborting the installation. >&2
            exit 1
          }
          echo ''${formatConfig:-} | ${localPkgs.nix-remote-run} $WAT_TARGET $FLAKE#nixosConfigurations.$machine.config.wat.build.installer.format.script
          unset formatConfig
        '';
      };
      install = {
        after = [ "early" "format" ];
        content = localPkgs: ''
          system_installable=".#nixosConfigurations.$machine.config.system.build.toplevel"

          echo Evaluate target system configuration
          ${localPkgs.nix-with-flakes} path-info --json $system_installable | ${localPkgs.jq}/bin/jq --raw-output ".[0].path" | read nixos_config_path

          echo Copy target system
          ${localPkgs.nix-with-flakes} copy --substitute-on-destination --to "ssh://$WAT_TARGET?remote-store=/mnt" $system_installable

          ${localPkgs.nix-remote-run} $WAT_TARGET $FLAKE#nixosConfigurations.$machine.config.wat.build.installer.activationScript $nixos_config_path
        '';
      };
    };

    wat.build.installer.launcher.script = genAttrs platforms.all (system: let
      localPkgs = import pkgs.path {
        inherit system;
        overlays = (toList (flakes.self.overlay or [])) ++ (toList (flakes.self.overlays.default or []));
      };
      fragments = types.dependencyDagOfSubmodule.toOrderedList cfg.installer.launcher.fragments;
    in pkgs.writeScriptBin "wat-installer-launcher-${hostname}" ''
      #!${localPkgs.zsh}/bin/zsh
      set -euo pipefail

      ${concatMapStringsSep "\n" (x: x.content localPkgs) fragments}
    '');

  };

}
