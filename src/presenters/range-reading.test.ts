// @spec openspec/specs/review/range-reading/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ClaimConflict, RangeReadingView, ReadingSummary } from "@/lib/bindings"
import { describeClaimConflict, describeUnproven, formatClaimConflict, formatReading, formatStackReading } from "@/presenters/range-reading"

const summary = (verdict: ReadingSummary["verdict"], unproven = 0, pending = false): ReadingSummary => ({ verdict, unproven, pending })

describe("the words of what a range leaves to read", () => {
  it("says the verdict of a reading on one line, with what cannot be proven and what waits for the index", () => {
    expect(
      [
        summary({ kind: "nothingToRead", proofs: ["formatting", "renames", "imports", "moves"] }),
        summary({ kind: "nothingToRead", proofs: [] }),
        summary({ kind: "testsOnly", toRead: 3, files: 2 }),
        summary({ kind: "docsOnly", toRead: 1, files: 1 }),
        summary({ kind: "toRead", toRead: 12, files: 5 }, 2),
        summary({ kind: "toRead", toRead: 0, files: 0 }, 1),
        summary({ kind: "toRead", toRead: 2, files: 1 }, 0, true),
      ].map(formatReading),
    ).toEqual([
      "Nothing to read · formatting, renamed names, imports, moves",
      "Nothing to read",
      "Tests only · 3 changes to read in 2 files",
      "Docs only · 1 change to read in 1 file",
      "12 changes to read in 5 files · 2 files not proven",
      "1 file not proven",
      "2 changes to read in 1 file · checking renames",
    ])
  })

  it("names the files that cannot be proven with why, for a hover", () => {
    const view: RangeReadingView = {
      reading: {
        roles: [],
        unproven: [
          { path: "icon.png", role: "resources", why: "binary" },
          { path: "Report.kt", role: "code", why: "renamesPending" },
          { path: "build.sh", role: "code", why: "noGrammar" },
        ],
        proofs: [],
      },
      summary: summary({ kind: "toRead", toRead: 1, files: 1 }, 2, true),
      conflict: null,
    }
    expect(describeUnproven(view)).toBe("Not proven: icon.png (binary), build.sh (no grammar)")
    expect(describeUnproven({ ...view, reading: { ...view.reading, unproven: [] } })).toBeNull()
  })

  it("words the author's word the diff proves wrong as the author's", () => {
    const labelled: ClaimConflict = { claim: { kind: "refactor", source: { kind: "label", name: "refactoring" } }, toRead: 3, files: 2 }
    const titled: ClaimConflict = { claim: { kind: "tests", source: { kind: "title", prefix: "test" } }, toRead: 1, files: 1 }
    expect([formatClaimConflict(labelled), formatClaimConflict(titled)]).toEqual(["Labelled refactoring · 3 changes to read", "test: in the title · 1 change to read"])
    expect(describeClaimConflict(labelled)).toBe(
      "The author calls it refactoring, but 3 changes of code in 2 files are left to read: no formatting, rename, import or move proves them",
    )
  })

  it("sums the layers of a stack", () => {
    expect(formatStackReading({ nothingToRead: 10, toReadLayers: 2, toRead: 7, unknown: 0, firstToRead: 3 })).toBe("10 with nothing to read · 2 to read · 7 changes")
    expect(formatStackReading({ nothingToRead: 1, toReadLayers: 0, toRead: 0, unknown: 2, firstToRead: null })).toBe("1 with nothing to read · 2 being read")
  })
})
