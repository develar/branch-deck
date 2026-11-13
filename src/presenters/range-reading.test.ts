// @spec openspec/specs/review/range-reading/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ClaimConflict, RangeReadingView, ReadingSummary } from "@/lib/bindings"
import {
  claimedLabel,
  describeClaimConflict,
  describeUnproven,
  formatClaimConflict,
  formatCompactReading,
  formatReading,
  formatRequestReading,
  formatStackReading,
  formatStackRowReading,
  orderLabels,
} from "@/presenters/range-reading"

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
    // a label says it in its own chip, first of the labels; a word of the title in a compact chip, its count in the hover
    expect([claimedLabel(labelled), claimedLabel(titled), claimedLabel(null)]).toEqual(["refactoring", null, null])
    expect(orderLabels(["proxy", "refactoring", "docs"], "refactoring")).toEqual(["refactoring", "proxy", "docs"])
    expect(orderLabels(["proxy"], null)).toEqual(["proxy"])
    expect([formatClaimConflict(labelled), formatClaimConflict(titled)]).toEqual([null, "test: in the title"])
    expect(describeClaimConflict(labelled)).toBe(
      "The author calls it refactoring, but 3 changes of code in 2 files are left to read: no formatting, rename, import or move proves them",
    )
  })

  it("sums the layers of a stack, each count of its own unit", () => {
    expect(
      [
        { layers: 12, nothingToRead: 10, toReadLayers: 2, toRead: 7, unknown: 0, firstToRead: 3 },
        { layers: 7, nothingToRead: 0, toReadLayers: 7, toRead: 14, unknown: 0, firstToRead: 0 },
        { layers: 3, nothingToRead: 1, toReadLayers: 0, toRead: 0, unknown: 2, firstToRead: null },
        { layers: 3, nothingToRead: 3, toReadLayers: 0, toRead: 0, unknown: 0, firstToRead: null },
        { layers: 3, nothingToRead: 2, toReadLayers: 1, toRead: 0, unknown: 0, firstToRead: 2 },
      ].map(formatStackReading),
    ).toEqual([
      "10 of 12 with nothing to read · 7 changes to read",
      "14 changes to read",
      "1 of 3 with nothing to read · 2 being read",
      "Nothing to read",
      "2 of 3 with nothing to read · 1 of 3 to read",
    ])
  })

  it("says a verdict compactly for a narrow list", () => {
    expect(
      [
        summary({ kind: "nothingToRead", proofs: ["formatting"] }),
        summary({ kind: "toRead", toRead: 4, files: 1 }, 0, true),
        summary({ kind: "testsOnly", toRead: 1, files: 1 }),
        summary({ kind: "docsOnly", toRead: 2, files: 2 }),
        summary({ kind: "toRead", toRead: 0, files: 0 }, 1),
      ].map(formatCompactReading),
    ).toEqual(["Nothing to read", "4 to read · checking", "Tests only · 1 to read", "Docs only · 2 to read", "1 not proven"])
  })

  it("says what a row of Your Turn reads ahead: checking, then the verdict of its request or the sum of its stack", () => {
    const view: RangeReadingView = {
      reading: { roles: [], unproven: [], proofs: ["formatting"] },
      summary: summary({ kind: "nothingToRead", proofs: ["formatting"] }),
      conflict: null,
    }
    expect([formatRequestReading({ kind: "checking" }), formatRequestReading({ kind: "read", view })]).toEqual(["Checking…", "Nothing to read · formatting"])
    expect([
      formatStackRowReading({ kind: "checking" }),
      formatStackRowReading({ kind: "read", summary: { layers: 12, nothingToRead: 10, toReadLayers: 2, toRead: 3, unknown: 0, firstToRead: 4 } }),
    ]).toEqual(["Checking…", "10 of 12 with nothing to read · 3 changes to read"])
  })
})
