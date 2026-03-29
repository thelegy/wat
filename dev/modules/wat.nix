{ inputs, ... }:
let
  outPath = ../..;
  watFallback = import outPath // {
    inherit outPath;
    __toString = _: outPath;
  };
in
{

  flake-file.inputs.wat.url = "github:input-output-hk/empty-flake";

  _module.args.wat = if inputs.wat.outputs != { } then inputs.wat else watFallback;

}
