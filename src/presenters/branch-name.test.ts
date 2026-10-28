// @spec spec/branch/move-commits.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { BranchNameCheck, BranchNameRules } from "@/lib/bindings"
import { presentBranchNameState } from "@/presenters/branch-name"

/** The typed names Rust checked, with the rules and its answer (`branch_name_cases_test.rs` of domain-policy writes it) */
const CASES = Object.values(
  import.meta.glob<(BranchNameCheck & { name: string; rules: BranchNameRules })[]>("/src/test-support/branch-names.cases.json", { eager: true, import: "default" }),
)[0]!

describe("presentBranchNameState", () => {
  it("words every answer of the oracle Rust writes as the field says it", () => {
    expect(CASES.map(({ name, rules, state }) => [name, rules.stacks === true, presentBranchNameState(state, rules).message])).toEqual([
      ["", false, ""],
      ["feature-auth", false, ""],
      ["  feature   auth  ", false, ""],
      ["a", false, "Branch name must be at least 2 characters"],
      [" a ", false, "Branch name must be at least 2 characters"],
      ["invalid@branch#name", false, "Use letters, numbers, -, _, ., / (cannot start with .)"],
      ["invalid@branch", true, "Use letters, numbers, -, _, . and / between a parent and its branch (cannot start with .)"],
      [".hidden", false, "Use letters, numbers, -, _, ., / (cannot start with .)"],
      ["auth/login", false, ""],
      ["auth/login", true, ""],
      ["auth/login", true, "A branch name has no /: use Stack On… to change its parent"],
      ["login", true, ""],
      ["feature-", false, "Not a valid branch name: no -, . or / at the ends, no -- , .. or //"],
      ["-feature", false, "Not a valid branch name: no -, . or / at the ends, no -- , .. or //"],
      ["feature--auth", false, "Not a valid branch name: no -, . or / at the ends, no -- , .. or //"],
      ["feature..auth", false, "Not a valid branch name: no -, . or / at the ends, no -- , .. or //"],
      ["auth//login", true, "Not a valid branch name: no -, . or / at the ends, no -- , .. or //"],
      ["auth/", true, "Not a valid branch name: no -, . or / at the ends, no -- , .. or //"],
      ["v1.2", false, ""],
      ["snake_case", false, ""],
    ])
  })

  it("marks only a valid name valid, and every name with words invalid", () => {
    for (const { state, rules } of CASES) {
      const shown = presentBranchNameState(state, rules)
      expect(shown.isValid).toBe(state === "valid")
      expect(shown.color === "error").toBe(Boolean(shown.message))
    }
  })
})
