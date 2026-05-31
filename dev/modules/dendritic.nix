{ inputs, ... }:
{
  imports = [
    (inputs.flake-file.flakeModules.dendritic or { })
  ];

  flake-file.inputs.flake-file.url = "github:denful/flake-file";

  flake-file.inputs.flake-compat.flake = false;
  flake-file.inputs.flake-compat.url = "github:NixOS/flake-compat";
}
