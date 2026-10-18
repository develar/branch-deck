import { describe, expect, it } from "vite-plus/test"
import type { CommitFile } from "@/lib/bindings"
import { filterFilesByText, formatNoFileMatch } from "@/lib/file-filter"

function file(path: string, oldPath: string | null = null): CommitFile {
  return { path, oldPath, status: oldPath === null ? "modified" : "renamed", additions: 1, deletions: 0, binary: false }
}

const files = [file("plugins/air/src/AcpSessionLiveState.kt"), file("plugins/air/testSrc/AcpPromptTest.kt"), file("spec/core/trace.spec.md", "spec/core/old-trace.md")]

describe("filterFilesByText", () => {
  it("keeps every file for a blank text", () => {
    expect(filterFilesByText(files, "  ")).toBe(files)
  })

  it("keeps the files with every word in their path, ignoring case", () => {
    expect(filterFilesByText(files, "AIR acp").map(it => it.path)).toEqual(["plugins/air/src/AcpSessionLiveState.kt", "plugins/air/testSrc/AcpPromptTest.kt"])
    expect(filterFilesByText(files, "testsrc prompt").map(it => it.path)).toEqual(["plugins/air/testSrc/AcpPromptTest.kt"])
  })

  it("finds a renamed file by its old path", () => {
    expect(filterFilesByText(files, "old-trace").map(it => it.path)).toEqual(["spec/core/trace.spec.md"])
  })

  it("keeps none when a word matches nothing", () => {
    expect(filterFilesByText(files, "air nothing")).toEqual([])
  })
})

describe("formatNoFileMatch", () => {
  it("quotes the trimmed text", () => {
    expect(formatNoFileMatch(" acp ")).toBe("No files match “acp”")
  })
})
