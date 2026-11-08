# Namespaced modules

Import WAT's flake module and configure `wat.namespace`. Dendritic flake-parts
modules can then request the configured helper library as the `wat` argument:

```nix
{ wat, lib, ... }:
{
  flake.modules.nixos.terminal = wat.mkModule [ "desktop" "terminal" ] (
    { cfg, pkgs, ... }:
    {
      options.package = lib.mkOption {
        type = lib.types.package;
        default = pkgs.foot;
      };
      config.environment.systemPackages = [ cfg.package ];
    }
  );
}
```

`mkModule namespace module` accepts an attribute set or a module function.
Functions receive the destination module system's normal arguments and `cfg`,
the configuration at the effective namespace. `config` still means the entire
destination configuration. `cfg` is local to this function, not an argument
installed globally or passed to imported modules.

The effective namespace is `wat.namespacePrefix ++ wat.namespace ++ namespace`.
Options are automatically placed there, including an `enable` option defaulting
to false. Configuration is applied only when enabled. Disabled configuration may
reference unavailable options beneath an existing top-level option namespace
(such as `programs`). As with the old wrapper, entirely unknown top-level
namespaces still cause an evaluation error. A wrapped module may
not set its own `enable`, even to false; set it from another module instead.

As with ordinary Nix modules, an attribute set without `options` or `config` is
configuration shorthand. `disabledModules`, `_class`, `_file`, and
`key` are module metadata and are preserved. `imports` (including the legacy
`require` spelling) is rejected: compose modules outside `wat.mkModule`, where
their activation behavior is explicit. With explicit
`options` or `config`, other configuration attributes must go inside `config`.
Use an ordinary module for module forms outside this API.

The same helper works under `flake.modules.nixos`, `flake.modules.homeManager`,
or other module classes. Its contents must use options available in that class.
Identical namespaces in different configurations have independent values and
enable switches. The helper does not register modules or enable them itself.

Additional helpers all take a relative namespace list:

- `wat.namespacePath namespace` returns the effective namespace list.
- `wat.toNamespace namespace value` places a value under that namespace.
- `wat.fromNamespace namespace attrs` reads that namespace from an attribute set, failing
  if it is absent.

Namespaces may be empty (to use the repository namespace directly); each supplied
segment must be a non-empty string. For occasional namespaced configuration,
reuse the same namespace with `mkModule` and `toNamespace`.

The helpers also remain accessible as `config.wat.lib`. The old file-discovery
wrapper retains its existing API, including explicitly lifted options and the
`mkTrivialModule` helper, for existing consumers.
