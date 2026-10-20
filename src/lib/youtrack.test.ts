import { describe, expect, it } from "vite-plus/test"
import { formatYouTrackHost, suggestYouTrackUrls, youTrackKeyOf, youTrackTokensPage } from "@/lib/youtrack"

const link = (linkRegexp: string) => ({ issueRegexp: "\\b[A-Z]+-\\d+\\b", linkRegexp })

describe("suggestYouTrackUrls", () => {
  it("offers the YouTrack of each issue page link, most recent repository first, each once", () => {
    const projects = [
      { path: "/work/monorepo", issueNavigationConfig: { links: [link("https://jira.example.com/browse/$0"), link("https://youtrack.example.com/issue/$0")] } },
      { path: "/work/plugin/", issueNavigationConfig: { links: [link("https://youtrack.example.com/issue/$0"), link("https://example.com/youtrack/issue/$1")] } },
      { path: "/work/empty" },
    ]
    expect(suggestYouTrackUrls(projects)).toEqual([
      { url: "https://youtrack.example.com", repository: "monorepo" },
      { url: "https://example.com/youtrack", repository: "plugin" },
    ])
  })
})

describe("youTrackKeyOf", () => {
  it("takes the key of a link to the issue page of the YouTrack", () => {
    expect(youTrackKeyOf("https://youtrack.example.com/issue/ABC-1", "https://youtrack.example.com")).toBe("ABC-1")
    expect(youTrackKeyOf("https://YouTrack.example.com/issue/ABC-1#focus", "https://youtrack.example.com")).toBe("ABC-1")
    expect(youTrackKeyOf("https://example.com/youtrack/issue/ABC-1", "https://example.com/youtrack")).toBe("ABC-1")
  })

  it("refuses a link of another host, path or scheme, and what is not a key", () => {
    for (const href of [
      "https://youtrack.example.com.evil.com/issue/ABC-1",
      "http://youtrack.example.com/issue/ABC-1",
      "https://youtrack.example.com/issues/ABC-1",
      "https://youtrack.example.com/issue/..%2Fadmin",
      "https://youtrack.example.com/issue/ABC-1/x",
      "https://jira.example.com/browse/ABC-1",
      "not a url",
    ]) {
      expect(youTrackKeyOf(href, "https://youtrack.example.com"), href).toBeNull()
    }
  })
})

describe("the pages of a YouTrack", () => {
  it("names the token page and the host", () => {
    expect(youTrackTokensPage("https://example.com/youtrack")).toBe("https://example.com/youtrack/users/me?tab=account-security")
    expect(formatYouTrackHost("https://example.com/youtrack")).toBe("example.com/youtrack")
  })
})
