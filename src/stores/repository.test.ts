// @spec spec/repository/settings.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { GitBranchPrefix } from "@/lib/bindings"
import { computeEffectiveBranchPrefix } from "@/stores/repository"
import type { BranchPrefixResult } from "@/stores/repository"

const loaded = (prefix: string, repositoryScoped: boolean): BranchPrefixResult => ({ status: "ok", data: { prefix, repositoryScoped } })
const notLoaded: BranchPrefixResult = { status: "error", error: { message: "Not loaded" } }
const cached = (prefix: string, repositoryScoped: boolean): GitBranchPrefix => ({ prefix, repositoryScoped })

// the cases of `resolve_branch_prefix` in crates/sync-core/src/branch_prefix_test.rs
describe("computeEffectiveBranchPrefix", () => {
  it("takes the repository's own git prefix, then the app's, then the global git prefix", () => {
    expect(computeEffectiveBranchPrefix("app", undefined, loaded("", false))).toBe("app")
    // the repository's own git prefix wins over the app's
    expect(computeEffectiveBranchPrefix("app", undefined, loaded("local", true))).toBe("local")
    // the app's prefix wins over the global git prefix
    expect(computeEffectiveBranchPrefix("app", undefined, loaded("global", false))).toBe("app")
    expect(computeEffectiveBranchPrefix(undefined, undefined, loaded("global", false))).toBe("global")
    expect(computeEffectiveBranchPrefix("  ", undefined, loaded("global", false))).toBe("global")
    expect(computeEffectiveBranchPrefix(undefined, undefined, loaded("", false))).toBe("")
  })

  it("names the branches as the backend does: no trailing slash", () => {
    expect(computeEffectiveBranchPrefix("app/", undefined, loaded("local/ ", true))).toBe("local")
    expect(computeEffectiveBranchPrefix(" app/", undefined, loaded("", false))).toBe("app")
  })

  it("takes the prefix cached for the repository until its load answers, then the loaded one", () => {
    expect(computeEffectiveBranchPrefix("app", cached("local", true), notLoaded)).toBe("local")
    expect(computeEffectiveBranchPrefix("app", cached("global", false), notLoaded)).toBe("app")
    expect(computeEffectiveBranchPrefix(undefined, cached("global", false), notLoaded)).toBe("global")
    // the local prefix was removed since the cache: the load wins
    expect(computeEffectiveBranchPrefix("app", cached("local", true), loaded("", false))).toBe("app")
  })
})
