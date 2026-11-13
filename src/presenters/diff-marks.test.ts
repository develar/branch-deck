// @spec openspec/specs/commit/quiet-changes/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { LineMove } from "@/lib/bindings"
import { describeMoveEnd, describeRenamedName, describeToRead, fileNameAmong, formatHiddenMatches, formatToRead, MOVE_END_LABEL, presentRemark } from "@/presenters/diff-marks"
import type { OtherFile, RenamedName } from "@/presenters/diff-marks"

const lines = (oldStart: number, oldEnd: number, newStart: number, newEnd: number) =>
  [
    [oldStart, oldEnd],
    [newStart, newEnd],
  ] as const

describe("presentRemark", () => {
  it("says what the lines of a run changed in and how far it reaches, by the new lines", () => {
    expect(presentRemark({ kind: "formatting", lines: lines(6, 7, 11, 14), open: null, move: null })).toEqual({
      label: "Formatting only",
      renamed: null,
      target: null,
      detail: "3 lines",
      more: null,
      reach: [],
      name: "Formatting only, 3 lines",
      toggle: null,
    })
    expect(presentRemark({ kind: "comments", lines: lines(27, 28, 36, 37), open: null, move: null })).toMatchObject({ label: "Comments only", detail: "1 line", toggle: null })
    expect(presentRemark({ kind: "renamed", lines: lines(2, 4, 2, 4), open: null, move: null }).label).toBe("Renamed only")
  })

  it("names the button of a note that opens by what a press does to the old lines", () => {
    expect(presentRemark({ kind: "formatting", lines: lines(6, 7, 11, 14), open: false, move: null }).toggle).toBe("Show the old line")
    expect(presentRemark({ kind: "formatting", lines: lines(6, 7, 11, 14), open: true, move: null }).toggle).toBe("Hide the old line")
    expect(presentRemark({ kind: "formatting", lines: lines(2, 42, 3, 43), open: false, move: null })).toMatchObject({
      label: "Formatting only",
      detail: "40 lines",
      toggle: "Show the 40 old lines",
    })
    expect(presentRemark({ kind: "formatting", lines: lines(2, 42, 3, 43), open: true, move: null }).toggle).toBe("Hide the 40 old lines")
  })
})

describe("the words of a move", () => {
  // 13 lines and the blank line before them leave the old lines 119..133 (120 to 133 as a reader counts) for the new
  // lines 209..223
  const move = (extra: Partial<LineMove> = {}): LineMove => ({
    oldStart: 120,
    newStart: 209,
    len: 13,
    indent: 0,
    blanksBefore: 1,
    blanksAfter: 0,
    edited: [],
    oldRanges: [],
    newRanges: [],
    ...extra,
  })

  it("says where the lines that left stand now, by the first line of that end, and what its buttons do", () => {
    const source = (open: boolean | null) => presentRemark({ kind: "moved", lines: lines(119, 133, 300, 300), open, move: { at: "source", move: move(), other: [209, 223] } })
    expect(source(false)).toEqual({
      label: "14 lines moved",
      renamed: null,
      target: { words: "to line 210", name: "Go to line 210, where the lines stand now", jump: true },
      detail: "",
      more: null,
      reach: [],
      name: "14 lines moved to line 210",
      toggle: "Show the 14 old lines",
    })
    expect([source(true).toggle, source(null).toggle]).toEqual(["Hide the 14 old lines", null])
    // one line, and the blank lines after the old lines, which stand before the new ones: the first line of the other
    // end as the view has it (`MoveEnd.other`), never counted here from the blank lines of the move
    const one = presentRemark({
      kind: "moved",
      lines: lines(7, 8, 3, 3),
      open: false,
      move: { at: "source", move: move({ oldStart: 7, newStart: 41, len: 1, blanksBefore: 0, blanksAfter: 2 }), other: [39, 42] },
    })
    expect([one.label, one.target?.words, one.toggle]).toEqual(["1 line moved", "to line 40", "Show the old line"])
    // blank lines that reach out of the block of the other end are no lines of it: the place is the line under its note
    const cut = presentRemark({
      kind: "moved",
      lines: lines(7, 8, 3, 3),
      open: false,
      move: { at: "source", move: move({ oldStart: 7, newStart: 41, len: 1, blanksBefore: 0, blanksAfter: 2 }), other: [40, 42] },
    })
    expect([cut.target?.words, cut.target?.name]).toEqual(["to line 41", "Go to line 41, where the lines stand now"])
  })

  it("says where the lines that stand now were, whether they were indented again and how many of them were edited on the way", () => {
    const destination = (extra: Partial<LineMove> = {}) =>
      presentRemark({ kind: "moved", lines: lines(300, 300, 209, 223), open: null, move: { at: "destination", move: move(extra), other: [119, 133] } })
    expect(destination()).toEqual({
      label: "Moved",
      renamed: null,
      target: { words: "from line 120", name: "Go to line 120, where the lines were", jump: true },
      detail: "",
      more: null,
      reach: [],
      name: "Moved from line 120",
      toggle: null,
    })
    expect([destination({ indent: 2 }).detail, destination({ indent: -4 }).detail, destination({ edited: [3] }).detail]).toEqual(["re-indented", "re-indented", "1 line edited"])
    expect(destination({ indent: 2, edited: [1, 4] })).toMatchObject({ detail: "re-indented · 2 lines edited", name: "Moved from line 120, re-indented, 2 lines edited" })
  })

  it("names the action that shows the other end of a move, and says where it goes from each end", () => {
    expect([MOVE_END_LABEL, describeMoveEnd("source"), describeMoveEnd("destination")]).toEqual([
      "Go to Other End of Move",
      "Show where the moved lines stand now",
      "Show where the moved lines were",
    ])
    // of a move between two files, with the file of the other end
    expect([describeMoveEnd("source", "Report.kt"), describeMoveEnd("destination", "Ledger.kt")]).toEqual([
      "Show where the moved lines stand now, in Report.kt",
      "Show where the moved lines were, in Ledger.kt",
    ])
  })
})

describe("the words of a move between two files", () => {
  const report: OtherFile = { path: "src/Report.kt", name: "Report.kt", shown: true }
  const ledger: OtherFile = { path: "src/Ledger.kt", name: "Ledger.kt", shown: true }
  // four lines left the old lines 54..58 of the ledger for the new lines 17..21 of the report
  const moved: LineMove = { oldStart: 54, newStart: 17, len: 4, indent: 0, blanksBefore: 0, blanksAfter: 0, edited: [], oldRanges: [], newRanges: [] }
  const source = (file: OtherFile) => presentRemark({ kind: "moved", lines: lines(54, 58, 60, 60), open: null, move: { at: "source", move: moved, other: [17, 21], file } })
  const destination = (file: OtherFile) =>
    presentRemark({ kind: "moved", lines: lines(20, 20, 17, 21), open: null, move: { at: "destination", move: moved, other: [54, 58], file } })

  it("names the file and the first line of the other end at both ends, and counts no line: every line of the move shows", () => {
    expect(source(report)).toEqual({
      label: "Moved",
      renamed: null,
      target: { words: "to Report.kt:18", name: "Go to line 18 of src/Report.kt, where the lines stand now", jump: true },
      detail: "",
      more: null,
      reach: [],
      name: "Moved to line 18 of src/Report.kt",
      toggle: null,
    })
    expect(destination(ledger)).toEqual({
      label: "Moved",
      renamed: null,
      target: { words: "from Ledger.kt:55", name: "Go to line 55 of src/Ledger.kt, where the lines were", jump: true },
      detail: "",
      more: null,
      reach: [],
      name: "Moved from line 55 of src/Ledger.kt",
      toggle: null,
    })
  })

  it("names a declaration the index says moved whole at both ends, by its name, before the file and the line of the other end", () => {
    const declared = (at: "source" | "destination", file: OtherFile) =>
      presentRemark({
        kind: "moved",
        lines: at === "source" ? lines(54, 58, 60, 60) : lines(20, 20, 17, 24),
        open: null,
        move: { at, move: moved, other: at === "source" ? [17, 24] : [54, 58], file, declaration: "reportTitle" },
      })
    const left = declared("source", report)
    expect([left.label, left.target, left.name]).toEqual([
      "Moved reportTitle",
      { words: "to Report.kt:18", name: "Go to line 18 of src/Report.kt, where reportTitle stands now", jump: true },
      "Moved reportTitle to line 18 of src/Report.kt",
    ])
    const stands = declared("destination", ledger)
    expect([stands.label, stands.target, stands.name]).toEqual([
      "Moved reportTitle",
      { words: "from Ledger.kt:55", name: "Go to line 55 of src/Ledger.kt, where reportTitle was", jump: true },
      "Moved reportTitle from line 55 of src/Ledger.kt",
    ])
  })

  it("says the same words of an end in a file the view does not show, as words that show nothing", () => {
    const hidden = source({ ...report, shown: false })
    expect([hidden.label, hidden.target]).toEqual(["Moved", { words: "to Report.kt:18", name: "Line 18 of src/Report.kt, where the lines stand now", jump: false }])
    expect(destination({ ...ledger, shown: false }).target).toEqual({ words: "from Ledger.kt:55", name: "Line 55 of src/Ledger.kt, where the lines were", jump: false })
  })

  it("names a file among the files of its subject by its name, with the directories that tell it from another of that name", () => {
    const paths = ["src/Ledger.kt", "src/api/Report.kt", "src/impl/Report.kt", "tools/src/impl/Report.kt", "Report.kt", "README.md"]
    expect(paths.map((_, index) => fileNameAmong(paths, index))).toEqual([
      "Ledger.kt",
      "api/Report.kt",
      "src/impl/Report.kt",
      "tools/src/impl/Report.kt",
      // a file at the root that every other of its name ends with: its whole path, the shortest that is its own
      "Report.kt",
      "README.md",
    ])
    // a file the list does not hold has no name
    expect(fileNameAmong(paths, 9)).toBe("")
  })
})

describe("the words of a rename", () => {
  const total: RenamedName = { old: "total", new: "sum", sites: 12 }
  const count: RenamedName = { old: "count", new: "size", sites: 3 }
  const flag: RenamedName = { old: "flag", new: "enabled", sites: 1 }
  const note = (renames: RenamedName[], open: boolean | null = null, hides: [number, number][] = []) =>
    presentRemark({ kind: "renamed", lines: lines(13, 13, 27, 27), open, move: null, renames: { renames, hides } })

  it("says the two spellings of a renamed name and how many places hold it, one site or many", () => {
    expect(note([total])).toEqual({
      label: "Renamed",
      renamed: [{ old: "total", new: "sum", sites: "12 sites" }],
      target: null,
      detail: "",
      more: null,
      reach: [],
      name: "Renamed total to sum, 12 sites",
      toggle: null,
    })
    expect(note([flag])).toMatchObject({ renamed: [{ old: "flag", new: "enabled", sites: "1 site" }], name: "Renamed flag to enabled, 1 site" })
  })

  it("words both renames of a note of two, and of three or more the first with the count of the others, each told for a hover, to a screen reader and in the name of the note", () => {
    // two: both worded, never "and 1 more" where the one name would stand
    expect(note([total, count])).toMatchObject({
      renamed: [
        { old: "total", new: "sum", sites: "12 sites" },
        { old: "count", new: "size", sites: "3 sites" },
      ],
      more: null,
      name: "Renamed total to sum, 12 sites, and count to size, 3 sites",
    })
    // three: the first worded, the two others counted
    expect(note([total, count, flag])).toMatchObject({
      renamed: [{ old: "total", new: "sum", sites: "12 sites" }],
      more: { words: "and 2 more", names: "count → size · 3 sites\nflag → enabled · 1 site", said: ": count to size, 3 sites; flag to enabled, 1 site" },
      name: "Renamed total to sum, 12 sites, and 2 more: count to size, 3 sites; flag to enabled, 1 site",
    })
    expect(note([total, count, flag, total]).more?.words).toBe("and 3 more")
  })

  it("names the button of a note that opens by the old lines of every quiet run of its renames, one line or many", () => {
    const hides: [number, number][] = [
      [13, 14],
      [16, 17],
      [19, 21],
    ]
    expect([note([total], false, hides).toggle, note([total], true, hides).toggle, note([total], false, [[13, 14]]).toggle]).toEqual([
      "Show the 4 old lines",
      "Hide the 4 old lines",
      "Show the old line",
    ])
    // a rename whose every line is read opens nothing
    expect(note([total], null).toggle).toBeNull()
  })

  it("says what a renamed name of a line was on the new side, and what it became on the old one", () => {
    expect([describeRenamedName("new", total), describeRenamedName("old", total)]).toEqual(["Renamed from total", "Renamed to sum"])
  })

  const store: OtherFile = { path: "src/Store.kt", name: "Store.kt", shown: true }
  const open: RenamedName = { old: "openStore", new: "connectStore", sites: 2, proof: "asked", declared: { file: store, line: 18 } }

  it("says after a rename which file of the subject declares the name, a button that shows it where the view shows that file", () => {
    expect(note([open])).toMatchObject({
      renamed: [{ old: "openStore", new: "connectStore", sites: "2 sites" }],
      reach: [
        { rename: 0, of: null, declared: { words: "declared in Store.kt", name: "Go to the declaration of connectStore in src/Store.kt", jump: true }, proof: "asked", said: "" },
      ],
      name: "Renamed openStore to connectStore, 2 sites, declared in Store.kt",
    })
    // a file the view does not show: the same words, which show nothing
    expect(note([{ ...open, declared: { file: { ...store, shown: false }, line: 18 } }]).reach[0]?.declared).toEqual({
      words: "declared in Store.kt",
      name: "connectStore is declared in src/Store.kt",
      jump: false,
    })
  })

  it("says of the owner of a declaration that moved under another owner that the change follows the move, and where the declaration went", () => {
    const report: OtherFile = { path: "src/Report.kt", name: "Report.kt", shown: true }
    const owner: RenamedName = { old: "Ledger", new: "Report", sites: 1, proof: "confirmed", declared: { file: report, line: 1, moved: "title" } }
    expect(note([owner])).toMatchObject({
      label: "Follows a move",
      renamed: [{ old: "Ledger", new: "Report", sites: "1 site" }],
      reach: [{ declared: { words: "title moved to Report.kt", name: "Go to title in src/Report.kt", jump: true }, proof: "confirmed", said: "all uses in this change" }],
      name: "Follows a move Ledger to Report, 1 site, title moved to Report.kt, all uses in this change",
    })
    expect(note([{ ...owner, declared: { file: { ...report, shown: false }, line: 1, moved: "title" } }]).reach[0]?.declared).toEqual({
      words: "title moved to Report.kt",
      name: "title stands in src/Report.kt now",
      jump: false,
    })
    // beside a rename, the note is of renames
    expect(note([open, owner]).label).toBe("Renamed")
  })

  it("says how far the index proved a rename: every use in the change, uses outside it, another meaning, more than the name, not all checked", () => {
    const said = (proof: RenamedName["proof"]) => note([{ ...total, proof }]).reach.map(it => [it.proof, it.said])
    expect([said("confirmed"), said("usesOutside"), said("otherMeaning"), said("otherDeclaration"), said("incomplete")]).toEqual([
      [["confirmed", "all uses in this change"]],
      [["usesOutside", "uses outside this change"]],
      [["otherMeaning", "changes what another name means"]],
      [["otherDeclaration", "more than the name changed"]],
      [["incomplete", "uses not all checked"]],
    ])
    // nothing to tell while the index is still to be asked, where none answers, and of a name it is not asked of
    expect([said("asked"), said("unavailable"), said(null), said(undefined)]).toEqual([[], [], [], []])
    // with the file that declares it, in the words of the note and in its name
    expect(note([{ ...open, proof: "confirmed" }])).toMatchObject({
      reach: [{ declared: { words: "declared in Store.kt" }, proof: "confirmed", said: "all uses in this change" }],
      name: "Renamed openStore to connectStore, 2 sites, declared in Store.kt, all uses in this change",
    })
    expect(note([{ ...total, proof: "usesOutside" }]).name).toBe("Renamed total to sum, 12 sites, uses outside this change")
  })

  it("counts the places of a name the file declares in every file of the subject that holds it, and those of the file alone where no other does", () => {
    // the declaration of the store and the two calls of the report: what the proof beside the count speaks of
    const declared: RenamedName = { old: "openStore", new: "connectStore", sites: 1, proof: "confirmed", across: { sites: 3, files: 2 } }
    expect(note([declared])).toMatchObject({
      renamed: [{ old: "openStore", new: "connectStore", sites: "3 sites in 2 files" }],
      reach: [{ declared: null, proof: "confirmed", said: "all uses in this change" }],
      name: "Renamed openStore to connectStore, 3 sites in 2 files, all uses in this change",
    })
    // no other file of the subject holds the name: the places of the file, as of any rename
    expect(note([{ ...declared, sites: 4, across: { sites: 4, files: 1 } }]).renamed).toEqual([{ old: "openStore", new: "connectStore", sites: "4 sites" }])
    expect(note([{ ...declared, across: null }]).renamed).toEqual([{ old: "openStore", new: "connectStore", sites: "1 site" }])
    // a counted rename says the same in its line of the hover and to a screen reader
    expect(note([total, count, declared]).more).toEqual({
      words: "and 2 more",
      names: "count → size · 3 sites\nopenStore → connectStore · 3 sites in 2 files · all uses in this change",
      said: ": count to size, 3 sites; openStore to connectStore, 3 sites in 2 files, all uses in this change",
    })
  })

  it("tells of which rename of a note of several the subject speaks, by its new name, and of a counted one in its line of the hover", () => {
    const two = note([total, { ...open, proof: "confirmed" }])
    expect(two.reach).toEqual([
      {
        rename: 1,
        of: "connectStore",
        declared: { words: "declared in Store.kt", name: "Go to the declaration of connectStore in src/Store.kt", jump: true },
        proof: "confirmed",
        said: "all uses in this change",
      },
    ])
    expect(two.name).toBe("Renamed total to sum, 12 sites, and openStore to connectStore, 2 sites, declared in Store.kt, all uses in this change")
    // three: the first is worded with what the subject says of it; the counted ones say theirs where they are listed
    const three = note([{ ...total, proof: "usesOutside" }, count, { ...open, proof: "incomplete" }])
    expect([three.reach.map(it => [it.rename, it.of, it.said]), three.more]).toEqual([
      [[0, "sum", "uses outside this change"]],
      {
        words: "and 2 more",
        names: "count → size · 3 sites\nopenStore → connectStore · 2 sites · declared in Store.kt · uses not all checked",
        said: ": count to size, 3 sites; openStore to connectStore, 2 sites, declared in Store.kt, uses not all checked",
      },
    ])
  })

  it("words a run of the kind renamed that names no rename by its kind, as any run", () => {
    expect(presentRemark({ kind: "renamed", lines: lines(2, 4, 2, 4), open: null, move: null, renames: null })).toMatchObject({
      label: "Renamed only",
      renamed: null,
      detail: "2 lines",
    })
  })
})

describe("the words of the find and of what is left to read", () => {
  it("names the hits in the old lines a note stands for", () => {
    expect(formatHiddenMatches(1, 1)).toBe("1 match in the old line")
    expect(formatHiddenMatches(1, 3)).toBe("1 match in the old lines")
    expect(formatHiddenMatches(4, 3)).toBe("4 matches in the old lines")
  })

  it("says how many changes of a file are left to read, short beside its counts and in full for a hover", () => {
    expect([formatToRead(10), formatToRead(1), formatToRead(0)]).toEqual(["10 to read", "1 to read", "Nothing to read"])
    expect([describeToRead(10, 11), describeToRead(1, 2)]).toEqual(["10 of 11 changes to read", "1 of 2 changes to read"])
    expect([describeToRead(0, 3), describeToRead(0, 1)]).toEqual(["None of the 3 changes needs reading", "The change needs no reading"])
  })
})
