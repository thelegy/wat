{ self, ... }:
with self.lib;


{ path, name?null, namespace?[] }:

let

  generatedName = replaceStrings [".nix"] [""] (baseNameOf path);

  moduleName = if isNull name then generatedName else name;

  moduleNamespace = namespace ++ [moduleName];

  applyIfFunction = o: arg: if isFunction o then o arg else o;

  additionalModuleArgs = rec {

    inherit moduleName;

    liftToNamespace = contents: foldr (a: b: {"${a}" = b;}) contents moduleNamespace;

    extractFromNamespace = o: foldl (a: b: a."${b}") o moduleNamespace;

    mkModule = { options?{}, config }: let
      moduleConfig = config;
      mkModule_ = { config, lib, ... }: let
        cfg = extractFromNamespace config;
        baseOptions = liftToNamespace {enable = mkEnableOption "Enable the ${moduleName} config layer";};
      in {
        options = recursiveUpdate baseOptions (applyIfFunction options cfg);
        config = mkIf cfg.enable (applyIfFunction moduleConfig cfg);
      };
    in { imports = [ mkModule_ ]; };

    mkTrivialModule = module: mkModule { config = _: module; };

  };

  filterFunctionArgs = attrs: removeAttrs attrs (attrNames additionalModuleArgs);

  wrapModule = module:
    if isFunction module then
    setFunctionArgs (moduleArgs: (module (additionalModuleArgs // moduleArgs)))
      (filterFunctionArgs (functionArgs module))
    else module;

in {
  name = moduleName;
  value = wrapModule (import path);
}
