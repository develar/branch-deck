# Proposal

## Why

accio made every directory with a `go.mod`, a `pyproject.toml`, a `pom.xml`, a Gradle build or a `.csproj` a module
root, so the test data of a JPS module (`goland/intellij-go/impl/testData/.../go.mod`) split the module into one module
per test case in a review (JetBrains/accio#782; 28 paths of idea-1 `e8b5bb3dd..008a0a1848`). accio's decision
2026-11-14-fixture-manifest-no-root (0702f80c0, 64e5997cc) answers such a manifest by the module around it, with the
answer's fields and `answers.modules` 1 unchanged. Branch Deck reads the answer as it did; the plan needs no code, only
the case held by a test against the accio of this machine.

## What Changes

- Capability `branch/review-plan`: the scenario of the test data of a module, whose build files make no module.
- The index of the tests of the plan (`test_support::declared_modules`) answers such a build file as accio does.
- docs/design/change-review.md (decision 30) and docs/design/symbol-dependencies.md no longer say that accio splits the
  test data of a module.
