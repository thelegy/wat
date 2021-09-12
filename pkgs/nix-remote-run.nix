{ writeScript, zsh, nix-with-flakes }:

writeScript "nix-remote-run" ''
  #!${zsh}/bin/zsh
  set -e -u -o pipefail

  forcePty=""
  if [[ "$1" == "-t" ]]; then
    shift
    forcePty="-t"
  fi

  ${nix-with-flakes} build --no-link "$2"
  resultPath="$(${nix-with-flakes} path-info "$2")"

  targetMachine="$1"
  shift 2

  ${nix-with-flakes} copy --to "ssh://$targetMachine" "$resultPath"
  ssh $forcePty "$targetMachine" -- "$resultPath" "$@"
''
