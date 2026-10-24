import { describe, expect, it } from "vite-plus/test"
import { ProjectMetadataSchema } from "@/stores/settings/repositorySettings"

describe("ProjectMetadataSchema", () => {
  it("keeps the issue links of the repository", () => {
    const issueNavigationConfig = { links: [{ issueRegexp: "ABC-\\d+", linkRegexp: "https://issues.example.com/issue/$0" }] }
    expect(ProjectMetadataSchema.parse({ path: "/repo", issueNavigationConfig })).toEqual({ path: "/repo", issueNavigationConfig })
  })

  it("keeps no field a sync records: those are the state of the repository", () => {
    expect(ProjectMetadataSchema.parse({ path: "/repo", lastSyncTime: 1, lastBranchCount: 2, updateTimings: { sync: [3] }, lastFetchTime: 4 })).toEqual({
      path: "/repo",
      lastFetchTime: 4,
    })
  })

  it("drops malformed issue links, not the project", () => {
    expect(ProjectMetadataSchema.parse({ path: "/repo", parkedCollapsed: true, issueNavigationConfig: { links: [{ issueRegexp: 1 }] } })).toEqual({
      path: "/repo",
      parkedCollapsed: true,
    })
    expect(ProjectMetadataSchema.parse({ path: "/repo", issueNavigationConfig: null })).toEqual({ path: "/repo" })
  })
})

describe("cachedBranchPrefix", () => {
  const parse = (cachedBranchPrefix: unknown) => ProjectMetadataSchema.parse({ path: "/repo", cachedBranchPrefix }).cachedBranchPrefix

  it("keeps the prefix the last resolve answered, with its source", () => {
    expect(parse({ prefix: "jane", source: "app_setting" })).toEqual({ prefix: "jane", source: "app_setting" })
    expect(parse({ prefix: "own", source: "repository_config" })).toEqual({ prefix: "own", source: "repository_config" })
  })

  it("reads the git prefix cached before, a text or with its scope, as a prefix of its git config", () => {
    expect(parse("jane")).toEqual({ prefix: "jane", source: "global_config" })
    expect(parse({ prefix: "jane", repositoryScoped: true })).toEqual({ prefix: "jane", source: "repository_config" })
    expect(parse({ prefix: "jane", repositoryScoped: false })).toEqual({ prefix: "jane", source: "global_config" })
  })

  it("keeps no empty prefix and drops a malformed one, not the project", () => {
    expect(parse("")).toBeUndefined()
    expect(parse({ prefix: "", source: "none" })).toBeUndefined()
    expect(parse({ prefix: "", repositoryScoped: true })).toBeUndefined()
    expect(parse(undefined)).toBeUndefined()
    expect(parse({ prefix: "jane", source: "elsewhere" })).toBeUndefined()
    expect(ProjectMetadataSchema.parse({ path: "/repo", parkedCollapsed: true, cachedBranchPrefix: { prefix: 1 } })).toEqual({ path: "/repo", parkedCollapsed: true })
  })
})
