{ lib
, inxi
, jq
, util-linux
, writeScript
, zsh
, nixos-generate-config
}: with lib;

writeScript "wat-gather-script" ''
  #!${zsh}/bin/zsh
  set -e -u -o pipefail

  lsblkOutput="$(${util-linux}/bin/lsblk --json --output-all)"
  inxiOutput="$(${inxi}/bin/inxi --output json --output-file print -v8)"
  hwConfiguration="$(${nixos-generate-config}/bin/nixos-generate-config --no-filesystems --show-hardware-config|${jq}/bin/jq -Rs .)"

  ${jq}/bin/jq <<EOF
  {
    "lsblk": $lsblkOutput,
    "inxi": $inxiOutput,
    "hwConfiguration": $hwConfiguration
  }
  EOF
''
