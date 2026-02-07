{ writeScript, zsh, nix-with-flakes }:

writeScript "nix-remote-run" ''
  #!${zsh}/bin/zsh
  set -e -u -o pipefail

  forcePty=""
  while true; do
    if [[ "$1" == "-t" ]]; then
      shift
      forcePty="-t"
    elif [[ "$1" == "-b" ]]; then
      binary="$2"
      shift 2
    else
      break
    fi
  done

  ${nix-with-flakes} build --no-write-lock-file --no-link "$2"
  resultPath="$(${nix-with-flakes} path-info --no-write-lock-file "$2")"

  targetMachine="$1"
  shift 2

  ${nix-with-flakes} copy --no-write-lock-file --to "ssh://$targetMachine" "$resultPath"
  if [[ -n "''${binary:-}" ]]; then
    ssh $forcePty "$targetMachine" -- "$resultPath/bin/$binary" "$@"
  else
    ssh $forcePty "$targetMachine" -- "$resultPath" "$@"
  fi
''
