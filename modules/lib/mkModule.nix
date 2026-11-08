{
  config,
  lib,
  withSystem,
  ...
}:
let
  settings = config.wat;

  namespacePath =
    namespace:
    if !(builtins.isList namespace && lib.all (s: builtins.isString s && s != "") namespace) then
      throw "wat: namespace must be a list of non-empty strings"
    else
      settings.namespacePrefix ++ settings.namespace ++ namespace;

  toNamespace = namespace: lib.setAttrByPath (namespacePath namespace);
  fromNamespace = namespace: lib.getAttrFromPath (namespacePath namespace);

  # Gate branches without pushing mkIf into potentially nonexistent options.
  # The enable path itself must never depend on its own value.
  gateModuleConfig =
    enabled: segments: attrs:
    if segments == [ ] then
      throw "A module may never enable itself"
    else if attrs ? _type then
      if attrs ? content then
        attrs // { content = gateModuleConfig enabled segments attrs.content; }
      else if attrs ? contents then
        attrs // { contents = map (gateModuleConfig enabled segments) attrs.contents; }
      else
        throw "wat: cannot gate module property ${attrs._type}"
    else
      lib.mapAttrs (
        key: value:
        if key != lib.head segments then
          lib.mkMerge (lib.optional enabled value)
        else
          gateModuleConfig enabled (lib.tail segments) value
      ) attrs;

  mkModule =
    namespace: module:
    let
      path = namespacePath namespace;
      location = "wat.mkModule ${lib.showAttrPath path}";
      argumentNames =
        if lib.isFunction module then
          builtins.removeAttrs (lib.functionArgs module) [
            "cfg"
            "inputs'"
            "self'"
          ]
        else
          { };
      wrapped =
        args:
        let
          cfg = fromNamespace namespace args.config;
          primed = withSystem args.pkgs.stdenv.hostPlatform.system (
            { inputs', self', ... }:
            {
              inherit inputs' self';
            }
          );
          body =
            if lib.isFunction module then
              module (
                args
                // {
                  inherit cfg;
                  inherit (primed) inputs' self';
                }
              )
            else
              module;
          explicit = body ? options || body ? config;
          metadata = [
            "_class"
            "_file"
            "key"
            "disabledModules"
          ];
          unsupported = builtins.removeAttrs body (
            metadata
            ++ [
              "options"
              "config"
            ]
          );
          contents =
            if explicit then
              if unsupported != { } then
                throw "${location}: move configuration attributes into config when using options or config"
              else
                body.config or { }
            else
              builtins.removeAttrs body metadata;
        in
        if body ? imports || body ? require then
          throw "${location}: imports are not supported inside wat.mkModule; compose modules outside the wrapper"
        else
          builtins.intersectAttrs (lib.genAttrs metadata (_: null)) body
          // {
            _file = body._file or location;
            options = toNamespace namespace (
              lib.recursiveUpdate { enable = lib.mkEnableOption "the ${lib.showAttrPath path} config layer"; } (
                body.options or { }
              )
            );
            config = gateModuleConfig cfg.enable (path ++ [ "enable" ]) contents;
          };
    in
    lib.setFunctionArgs wrapped (
      argumentNames
      // {
        config = false;
        pkgs = false;
      }
    );

  # Retain the legacy wrapper's extra import level during migrations.
  mkModuleWrapped = namespace: module: {
    _file = "wat.mkModuleWrapped ${lib.showAttrPath (namespacePath namespace)}";
    imports = [ (mkModule namespace module) ];
  };
in
{
  wat.lib = {
    inherit
      namespacePath
      toNamespace
      fromNamespace
      mkModule
      mkModuleWrapped
      ;
  };
  wat.build = { inherit gateModuleConfig; };
  _module.args.wat = settings.lib;
}
