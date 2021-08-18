{ self, ... }:
with self.lib;


eachDefaultSystem (system: pkgs: rec {

  packages.wat-deploy-tools = pkgs.runCommand "wat-deploy-tools" {} ''
    mkdir -p $out/bin
    shopt -s extglob
    cp ${./wat-deploy-tools}/!(util.zsh) $out/bin
    for file in $out/bin/*; do
      export utilPath=${./wat-deploy-tools/util.zsh}
      substituteAllInPlace $file
    done
  '';

  packages.wat-deploy-tools-envrc = pkgs.writeText "wat-deploy-tools-envrc" ''
    PATH_add ${packages.wat-deploy-tools}/bin
  '';

  defaultPackage = packages.wat-deploy-tools;

})
