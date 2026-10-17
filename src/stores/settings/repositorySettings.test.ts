import { describe, expect, it } from "vite-plus/test"
import { ProjectMetadataSchema } from "@/stores/settings/repositorySettings"

describe("ProjectMetadataSchema", () => {
  it("keeps the issue links of the repository", () => {
    const issueNavigationConfig = { links: [{ issueRegexp: "ABC-\\d+", linkRegexp: "https://issues.example.com/issue/$0" }] }
    expect(ProjectMetadataSchema.parse({ path: "/repo", issueNavigationConfig })).toEqual({ path: "/repo", issueNavigationConfig })
  })

  it("drops malformed issue links, not the project", () => {
    expect(ProjectMetadataSchema.parse({ path: "/repo", lastSyncTime: 1, issueNavigationConfig: { links: [{ issueRegexp: 1 }] } })).toEqual({ path: "/repo", lastSyncTime: 1 })
    expect(ProjectMetadataSchema.parse({ path: "/repo", issueNavigationConfig: null })).toEqual({ path: "/repo" })
  })
})

describe("cachedBranchPrefix", () => {
  const parse = (cachedBranchPrefix: unknown) => ProjectMetadataSchema.parse({ path: "/repo", cachedBranchPrefix }).cachedBranchPrefix

  it("keeps the prefix with whether it is the repository's own", () => {
    expect(parse({ prefix: "jane", repositoryScoped: true })).toEqual({ prefix: "jane", repositoryScoped: true })
    expect(parse({ prefix: "jane", repositoryScoped: false })).toEqual({ prefix: "jane", repositoryScoped: false })
  })

  it("reads a text, as before the scope was kept, as a prefix of the global config", () => {
    expect(parse("jane")).toEqual({ prefix: "jane", repositoryScoped: false })
  })

  it("keeps no empty prefix and drops a malformed one, not the project", () => {
    expect(parse("")).toBeUndefined()
    expect(parse({ prefix: "", repositoryScoped: true })).toBeUndefined()
    expect(parse(undefined)).toBeUndefined()
    expect(ProjectMetadataSchema.parse({ path: "/repo", lastSyncTime: 1, cachedBranchPrefix: { prefix: 1 } })).toEqual({ path: "/repo", lastSyncTime: 1 })
  })
})
