{ config, lib, ... }:
let

  cfg = config.wat;

  wat.build = {
    inherit wrapModule;
  };

  gateModuleConfig = cfg.build.gateModuleConfig;

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

              config = gateModuleConfig cfg.enable (moduleNamespace ++ [ "enable" ]) (
                applyIfFunction moduleConfig cfg
              );

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
