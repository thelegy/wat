# Namespaced modules

Import WAT's flake module and configure `wat.namespace`. Dendritic flake-parts
modules can then request the configured helper library as the `wat` argument:

```nix
{ wat, lib, ... }:
{
  flake.modules.nixos.terminal = wat.mkModule [ "desktop" "terminal" ] (
    { cfg, pkgs, ... }:
    {
      namespaceOptions.package = lib.mkOption {
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

Functions also receive flake-parts' `inputs'` and `self'`, resolved through
`withSystem` for the destination's `pkgs.stdenv.hostPlatform.system`. These
values include the flake's configured per-system input projections. They are
resolved lazily, so modules that do not use them need neither `pkgs` nor
`withSystem`. Like `cfg`, these arguments are local to the wrapped function.

The effective namespace is `wat.namespacePrefix ++ wat.namespace ++ namespace`.
`namespaceOptions` are automatically placed there, including an `enable` option
defaulting to false. `options` declares options at their ordinary, absolute paths.
Both sets of declarations are merged and remain available when the module is disabled.
Existing callers using `options` for namespaced declarations should rename it to
`namespaceOptions`. Configuration is applied only when enabled. Disabled configuration may
reference unavailable options beneath an existing top-level option namespace
(such as `programs`). As with the old wrapper, entirely unknown top-level
namespaces still cause an evaluation error. A wrapped module may
not set its own `enable`, even to false; set it from another module instead.

As with ordinary Nix modules, an attribute set without `options`, `namespaceOptions`, or `config` is
configuration shorthand. `disabledModules`, `_class`, `_file`, and
`key` are module metadata and are preserved. `imports` (including the legacy
`require` spelling) is rejected: compose modules outside `wat.mkModule`, where
their activation behavior is explicit. With explicit
`options`, `namespaceOptions`, or `config`, other configuration attributes must go inside `config`.
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

For migrations from the file-discovery wrapper, use
`wat.mkModuleWrapped namespace module`. It accepts the same arguments as
`mkModule`, but adds the legacy wrapper's extra `imports` level so that module
ordering is preserved. For example:

```nix
wat.mkModuleWrapped [ "terminal" ] {
  config.environment.systemPackages = [ pkgs.foot ];
}
```

Keep the module's position in the surrounding imports unchanged and verify
machine output hashes during migration. Once migrated, switch
`mkModuleWrapped` to `mkModule` in a separate change so any ordering and hash
changes are easy to review. The helper preserves import depth; changes to the
module contents or surrounding composition can still affect hashes.
