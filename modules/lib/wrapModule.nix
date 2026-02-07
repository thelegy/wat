{ config, lib, ... }:
let

  cfg = config.wat;

  wat.build = {
    inherit wrapModule;
  };

  namespace = cfg.namespacePrefix ++ cfg.namespace;

  getModuleName =
    {
      path,
      name ? null,
    }:
    let
      generatedName = lib.replaceStrings [ ".nix" ] [ "" ] (baseNameOf path);
      moduleName = if isNull name then generatedName else name;
    in
    moduleName;

  additionalModuleArgs =
    {
      path,
      name,
    }:
    let

      moduleName = name;

      moduleNamespace = namespace ++ [ name ];

      applyIfFunction = o: arg: if lib.isFunction o then o arg else o;

      liftToNamespace = contents: lib.foldr (a: b: { "${a}" = b; }) contents moduleNamespace;

      extractFromNamespace = o: lib.foldl (a: b: a."${b}") o moduleNamespace;

      mkModule =
        {
          options ? { },
          config,
        }:
        let
          moduleConfig = config;
          mkModule_ =
            { config, lib, ... }:
            let
              cfg = extractFromNamespace config;
              baseOptions = liftToNamespace { enable = lib.mkEnableOption "the ${moduleName} config layer"; };
            in
            {

              _file = path;

              options = lib.recursiveUpdate baseOptions (applyIfFunction options cfg);

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
              config =
                let
                  #disableModule : [string] -> arrtset -> attrset
                  disableModule =
                    segments: attrs:
                    if lib.length segments <= 0 then
                      throw "A module may never enable itself"
                    else if (attrs ? _type) then
                      if (attrs ? content) then
                        (attrs // { content = disableModule segments attrs.content; })
                      else if (attrs ? contents) then
                        (attrs // { contents = map (x: disableModule segments x) attrs.contents; })
                      else
                        throw "Don't know how to handle _type ${attrs._type}"
                    else
                      lib.mapAttrs (
                        k: v:
                        if k != lib.head segments then
                          lib.mkMerge (lib.optional cfg.enable v)
                        else
                          disableModule (lib.tail segments) v
                      ) attrs;
                in
                disableModule (moduleNamespace ++ [ "enable" ]) (applyIfFunction moduleConfig cfg);

            };
        in
        {
          imports = [ mkModule_ ];
        };

      mkTrivialModule = module: mkModule { config = _: module; };

    in
    {
      inherit
        moduleName
        liftToNamespace
        extractFromNamespace
        mkModule
        mkTrivialModule
        ;
    };

  wrapModule =
    x@{ path, ... }:
    let
      name = getModuleName x;
      args = additionalModuleArgs { inherit path name; };
      filterFunctionArgs = attrs: removeAttrs attrs (lib.attrNames args);
      module = import path;
      value =
        if lib.isFunction module then
          lib.setFunctionArgs (moduleArgs: (module (args // moduleArgs))) (
            filterFunctionArgs (lib.functionArgs module)
          )
        else
          module;
    in
    {
      inherit name value;
    };

in
{
  inherit wat;
}
