# Proposal

## Why

The `BUILD.bazel` a generator writes for a new module is `generated` now (its change lies in the regions the generator
writes), no longer the build model by its name, so the mechanical group of a large step that held it was named
"Mechanical", not "Wiring", though the file still wires the module in.

## What Changes

- The mechanical group of a step is "Wiring" while it holds the build model, the lists of modules and the build files
  whose change lies in the regions their generator writes (`GeneratedBy` `region`).
