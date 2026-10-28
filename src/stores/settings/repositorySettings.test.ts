import { describe, expect, it } from "vite-plus/test"
import { ProjectMetadataSchema } from "@/stores/settings/repositorySettings"

describe("ProjectMetadataSchema", () => {
  it("keeps the issue links of the repository", () => {
    const issueNavigationConfig = { links: [{ issueRegexp: "ABC-\\d+", linkRegexp: "https://issues.example.com/issue/$0" }] }
    expect(ProjectMetadataSchema.parse({ path: "/repo", issueNavigationConfig })).toEqual({ path: "/repo", issueNavigationConfig })
  })

  it("keeps no field a sync or a fetch records: those are the state of the repository", () => {
    expect(ProjectMetadataSchema.parse({ path: "/repo", lastSyncTime: 1, lastBranchCount: 2, updateTimings: { sync: [3] }, lastFetchTime: 4, landedHiddenUntil: 5 })).toEqual({
      path: "/repo",
    })
  })

  it("keeps no prefix a window resolved before: the view of the session of the repository names it", () => {
    expect(ProjectMetadataSchema.parse({ path: "/repo", cachedBranchPrefix: { prefix: "jane", source: "app_setting" } })).toEqual({ path: "/repo" })
  })

  it("drops malformed issue links, not the project", () => {
    expect(ProjectMetadataSchema.parse({ path: "/repo", parkedCollapsed: true, issueNavigationConfig: { links: [{ issueRegexp: 1 }] } })).toEqual({
      path: "/repo",
      parkedCollapsed: true,
    })
    expect(ProjectMetadataSchema.parse({ path: "/repo", issueNavigationConfig: null })).toEqual({ path: "/repo" })
  })
})
