lib:
with lib;

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
        baseOptions = liftToNamespace {enable = mkEnableOption "the ${moduleName} config layer";};
      in {

        _file = path;

        options = recursiveUpdate baseOptions (applyIfFunction options cfg);

        #config = mkIf cfg.enable (applyIfFunction moduleConfig cfg);
        # `mkIf` has the drawback, that it could get pushed down into not
        # existent options, resulting in an evaluation error.
        # Here we have the opportunity to solve this misery, as we can use some
        # kind of `if` without pushdown and then resolve the infinite recursion
        # by filtering out the attr path to the `enable` option.
        # The code is more complex, but it allows modules to be much more
        # resillient in scenarions, where they get loaded by different versions
        # of nixpkgs or with different addionional modules loaded, which is both
        # something this framework supports.
        config = let
          #disableModule : [string] -> arrtset -> attrset
          disableModule = segments: attrs:
            if length segments <= 0
            then throw "A module may never enable itself"
            else
              if (attrs ? _type)
              then
                if (attrs ? content)
                then (attrs // { content = disableModule segments attrs.content; })
                else
                  if (attrs ? contents)
                  then (attrs // { contents = map (x: disableModule segments x) attrs.contents; })
                  else throw "Don't know how to handle _type ${attrs._type}"
              else
                mapAttrs (k: v:
                  if k != head segments
                  then mkMerge (optional cfg.enable v)
                  else disableModule (tail segments) v
                ) attrs;
        in disableModule (moduleNamespace ++ ["enable"]) (applyIfFunction moduleConfig cfg);

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
