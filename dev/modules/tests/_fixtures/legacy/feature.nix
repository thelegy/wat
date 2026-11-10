{
  mkModule,
  liftToNamespace,
  lib,
  ...
}:
mkModule {
  options = liftToNamespace {
    amount = lib.mkOption {
      default = 23;
    };
  };
  config = cfg: { result = cfg.amount; };
}
