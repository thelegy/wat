{ wat-run-tests
, lib
, path
, system
, watLib
}:
with watLib;

let

  toOrderedList = types.dependencyDagOfSubmodule.toOrderedList;

  nixosSystem = import "${path}/nixos/lib/eval-config.nix";

  machineTest = module: (nixosSystem {
    inherit system;
    modules = [
      { options.output = mkOption { type = types.anything; }; }
      module
    ];
  }).config.output;

  sampleOption = {
    options.sample = mkOption {
      type = types.dependencyDagOfSubmodule {
        options.value = mkOption {
          type = types.anything;
        };
      };
    };
  };

in
wat-run-tests {

  testTieBreaker = machineTest ({ config, ... }: {
    imports = [ sampleOption ];
    sample = {
      a.value = 1;
      c.value = 2;
      b.value = 3;
    };
    output = {
      expr = map (x: x.value) (toOrderedList config.sample);
      expected = [ 1 3 2 ];
    };
  });

  testAfter = machineTest ({ config, ... }: {
    imports = [ sampleOption ];
    sample = {
      a.value = 1;
      a.after = [ "c" ];
      b.value = 2;
      c.value = 3;
    };
    output = {
      expr = map (x: x.value) (toOrderedList config.sample);
      expected = [ 3 1 2 ];
    };
  });

  testBefore = machineTest ({ config, ... }: {
    imports = [ sampleOption ];
    sample = {
      a.value = 1;
      b.value = 2;
      c.value = 3;
      c.before = [ "a" ];
    };
    output = {
      expr = map (x: x.value) (toOrderedList config.sample);
      expected = [ 3 1 2 ];
    };
  });

  testTrivialLoop = machineTest ({ config, ... }: {
    imports = [ sampleOption ];
    sample = {
      a.value = 1;
      a.after = [ "a" ];
    };
    output = {
      expr = builtins.tryEval (toOrderedList config.sample);
      expected = { success = false; value = false; };
    };
  });

  testLoop = machineTest ({ config, ... }: {
    imports = [ sampleOption ];
    sample = {
      a.value = 1;
      b.value = 2;
      b.after = [ "a" ];
      b.before = [ "a" ];
    };
    output = {
      expr = builtins.tryEval (toOrderedList config.sample);
      expected = { success = false; value = false; };
    };
  });

  testMutualLoop = machineTest ({ config, ... }: {
    imports = [ sampleOption ];
    sample = {
      a.value = 1;
      a.after = [ "b" ];
      b.value = 2;
      b.after = [ "c" ];
      c.value = 3;
      c.after = [ "a" ];
    };
    output = {
      expr = builtins.tryEval (toOrderedList config.sample);
      expected = { success = false; value = false; };
    };
  });

  testLessConstrainedOrder = machineTest ({ config, ... }: {
    imports = [ sampleOption ];
    sample = {
      a.value = 1;
      b.value = 2;
      b.after = [ "a" ];
      c.value = 3;
      d.value = 4;
      d.after = [ "a" ];
    };
    output = {
      expr = map (x: x.value) (toOrderedList config.sample);
      expected = [ 1 2 3 4 ];
    };
  });

  testIgnoreDisabled = machineTest ({ config, ... }: {
    imports = [ sampleOption ];
    sample = {
      a.value = 1;
      b.value = 2;
      b.enabled = false;
      c.value = 3;
    };
    output = {
      expr = map (x: x.value) (toOrderedList config.sample);
      expected = [ 1 3 ];
    };
  });

  testDisabledApplyOrderEffects = machineTest ({ config, ... }: {
    imports = [ sampleOption ];
    sample = {
      a.value = 1;
      b.value = 2;
      b.after = [ "c" ];
      b.before = [ "a" ];
      b.enabled = false;
      c.value = 3;
    };
    output = {
      expr = map (x: x.value) (toOrderedList config.sample);
      expected = [ 3 1 ];
    };
  });


}
