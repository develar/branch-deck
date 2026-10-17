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
