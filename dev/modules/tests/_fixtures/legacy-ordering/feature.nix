{ mkModule, ... }:
mkModule {
  config = _: { ordered = [ "migrated" ]; };
}
