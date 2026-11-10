import { describe, expect, it } from "vite-plus/test"
import type {
  CommitBodyBlock,
  CommitBodyPart,
  Concern,
  ContractEntry,
  FileFacts,
  Mention,
  ProseParagraph,
  ReviewPlan,
  ReviewStep,
  ReviewUnit,
  StoryCommit,
  StoryPlace,
  StepTest,
  StepTests,
  ModuleShape,
} from "@/lib/bindings"
import { plainTitle } from "@/lib/commit"
import { joinNames } from "@/lib/text"
import { MORE } from "@/presenters/commit-body"
import { placeCommits } from "@/presenters/review-steps"
import { mentionWords, presentGroupOpenings, presentOpenings, presentPrologue, presentStepFacts, presentStory, storyMarks } from "@/presenters/review-story"
import type { ContractFile, ContractParagraph, FactList, StepFacts, StoryView } from "@/presenters/review-story"
import type { NarrativeSection } from "@/presenters/review-units"
import { fileFacts, moduleShape, reviewPlan, reviewStep, shownForm, stepCommit, stepGroup } from "@/test-support/review-step"

const commit = stepCommit
const step = reviewStep

const paragraph = (text: string): CommitBodyBlock => ({ type: "paragraph", lines: [text] })
const label = (text: string, kind: "plain" | "verification" | "lookClosely" = "plain"): CommitBodyBlock => ({ type: "label", text, kind, runIn: false })
const list = (...items: string[]): CommitBodyBlock => ({ type: "list", ordered: false, start: 1, items })

/** A part of a body as Rust cuts it (`sync_utils::commit_body`): its first block, the block after its last, folded or not */
type Part = readonly [start: number, end: number, folded: boolean]

/**
 * The story of the commit of `subject`: its blocks, the lead when it opens with prose, and its `parts` written out as
 * Rust cuts that body, each what its first block makes it (the TS tests never encode the grammar); one part of every
 * block by default, a body of one block or of one section. `told`: where Rust tells each part, with its commit by
 * default; `titled` the part whose label is the title of its step
 */
function story(
  subject: string,
  blocks: CommitBodyBlock[],
  parts: readonly Part[] = [[0, blocks.length, false]],
  told: readonly StoryPlace[] = [],
  titled: number | null = null,
): StoryCommit {
  const lead = blocks[0]?.type === "paragraph"
  const bodyParts = parts.map(([start, end, folded]): CommitBodyPart => {
    const first = blocks[start]
    const kind = first?.type === "label" || first?.type === "heading" ? "section" : lead && start === 0 && end === 1 ? "lead" : "run"
    return { start, end, kind, tone: first?.type === "label" ? first.kind : "plain", folded }
  })
  return {
    id: subject,
    title: plainTitle(subject),
    body: { blocks, trailers: [], lead, summary: lead ? 0 : null, parts: bodyParts, code: [] },
    mentions: [],
    parts: bodyParts.map((part, index) => ({ ...part, told: told[index] ?? { kind: "commit" }, titled: index === titled, removals: [] })),
    change: null,
  }
}

function plan(steps: ReviewStep[], commits: StoryCommit[], more: Partial<ReviewPlan> = {}): ReviewPlan {
  const base = reviewPlan()
  return reviewPlan({ steps: { ...base.steps, steps, commits }, ...more })
}

/** The text of the blocks of a part: a paragraph its lines, a list its items, a table its rows */
function textOf(view: StoryView): { lead: string | null; shown: string[][]; folded: string[] } {
  const words = (block: CommitBodyBlock) => {
    switch (block.type) {
      case "paragraph":
        return block.lines.join(" ")
      case "list":
        return block.items.join(" | ")
      case "table":
        return [block.header, ...block.rows].map(row => row.join(" | ")).join(" / ")
      default:
        return block.type
    }
  }
  return {
    lead: view.lead === null ? null : words(view.lead.block),
    shown: view.shown.map(part => [part.head?.block.type === "label" ? `${part.head.block.text}:` : "", ...part.blocks.map(it => words(it.block))]),
    folded: view.folded.map(it => it.name),
  }
}

/** A paragraph of prose the commits add whole, as Rust reads it (`review_plan::prose`) */
function prose(text: string, change: ProseParagraph["change"] = { kind: "added" }, links: ProseParagraph["links"] = []): ProseParagraph {
  return { text, form: { kind: "prose" }, code: [], links, change, excerpt: null }
}

/**
 * A row of a table as Rust reads it (`review_plan::prose`, `ProseForm::Row`): the cells of its header, a line break, then
 * its own, each ` | ` apart; the header row itself (no `header`) its cells alone
 */
function row(header: string[], cells: string[], change: ProseParagraph["change"] = { kind: "added" }): ProseParagraph {
  const ranges = (parts: string[], from: number) => {
    let at = from
    return parts.map((part, index) => {
      at += index === 0 ? 0 : 3
      const range = { start: at, end: at + part.length }
      at = range.end
      return range
    })
  }
  const head = header.join(" | ")
  const text = header.length === 0 ? cells.join(" | ") : `${head}\n${cells.join(" | ")}`
  const form: ProseParagraph["form"] = { kind: "row", header: ranges(header, 0), cells: ranges(cells, header.length === 0 ? 0 : head.length + 1) }
  return { text, form, code: [], links: [], change, excerpt: null }
}

/** The paragraphs of a section of What Changes, the rows of its tables among them in their order */
function paragraphsOf(entry: ContractFile["entries"][number]): ContractParagraph[] {
  return entry.blocks.flatMap(block => (block.kind === "paragraph" ? [block.paragraph] : [...(block.head === null ? [] : [block.head]), ...block.rows]))
}

function contract(path: string, heading: string | null, added: (string | ProseParagraph)[], provenBy: string[] = [], block = 0): ContractEntry {
  return {
    kind: "spec",
    unit: `other:${path}`,
    path,
    blocks: [block],
    heading,
    newSpec: null,
    deleted: false,
    paragraphs: added.map(it => (typeof it === "string" ? prose(it) : it)),
    provenBy,
  }
}

describe("the document of a review", () => {
  it("words What Changes: a file once, a section once with its tests, its paragraphs whole, a link showing a changed file or opening a page", () => {
    const changed = prose("The store keeps at most 500 projects and their focus.", { kind: "changed", inserted: [{ start: 37, end: 53 }], deleted: [], substitutions: [] }, [
      { start: 4, end: 9, target: "spec/store.spec.md" },
      { start: 10, end: 15, target: "https://example.com/keeps" },
      { start: 16, end: 18, target: "spec/elsewhere.spec.md" },
    ])
    const file = (path: string): FileFacts => ({ path, oldPath: null }) as FileFacts
    const view = presentPrologue(
      plan([], [], {
        files: [file("spec/a.spec.md"), file("spec/store.spec.md")],
        brief: {
          issue: null,
          substitutions: [],
          contract: [
            // two places of one section, which Rust tells apart by their blocks: one section, its tests once
            contract("spec/a.spec.md", "Storage", [changed], ["tests/restart.json"], 0),
            contract(
              "spec/a.spec.md",
              "Storage",
              ["The focus is a fact the window derives.", prose("Old words.", { kind: "removed" })],
              ["tests/restart.json", "tests/focus.json"],
              2,
            ),
            contract("spec/a.spec.md", "Requirements", ["The window opens."], [], 4),
            { ...contract("spec/new.spec.md", null, []), blocks: [], newSpec: { description: null, rules: 3 } },
            { ...contract("spec/old.spec.md", null, []), blocks: [], deleted: true },
          ],
        },
      }),
      false,
    )
    expect(
      view.contract.map(it => [
        it.path,
        it.entries.map(entry => [entry.heading, paragraphsOf(entry).map(paragraph => [paragraph.text, paragraph.change?.kind ?? null]), entry.provenBy]),
      ]),
    ).toEqual([
      [
        "spec/a.spec.md",
        [
          [
            "Storage",
            [
              ["The store keeps at most 500 projects and their focus.", "changed"],
              ["The focus is a fact the window derives.", "added"],
              ["Old words.", "removed"],
            ],
            "Proven by restart.json and focus.json",
          ],
          ["Requirements", [["The window opens.", "added"]], null],
        ],
      ],
      ["spec/new.spec.md", [[null, [["A new spec of 3 rules", null]], null]]],
      ["spec/old.spec.md", [[null, [["Deleted", null]], null]]],
    ])
    // the line a fold shows: of a changed file its changes and their sections, of a new spec its requirements
    expect(view.contract.map(it => it.summary)).toEqual(["4 changes in Storage and Requirements", "New spec (3 requirements)", "Deleted"])
    // a link to a changed file shows it at its top, one to a page opens it, one to a file the review does not show is text
    expect(paragraphsOf(view.contract[0]!.entries[0]!)[0]!.links).toEqual([
      { start: 4, end: 9, place: { path: "spec/store.spec.md", side: "new", line: null, name: null }, href: null },
      { start: 10, end: 15, place: null, href: "https://example.com/keeps" },
      { start: 16, end: 18, place: null, href: null },
    ])
  })

  it("words the rows of a table under their header once, a changed header row heading them, a paragraph between two tables apart", () => {
    const limits = ["Limit", "Value"]
    const owners = ["Topic", "Owner"]
    const view = presentPrologue(
      plan([], [], {
        files: [{ path: "spec/a.spec.md", oldPath: null } as FileFacts],
        brief: {
          issue: null,
          substitutions: [],
          contract: [
            contract("spec/a.spec.md", "Limits", [
              "The limits of the window.",
              row(limits, ["Tool windows", "40"]),
              row(limits, ["Projects", "600"], { kind: "changed", inserted: [{ start: 26, end: 29 }], deleted: [{ at: 26, text: "500" }], substitutions: [] }),
              "The owners of the window.",
              row([], owners, { kind: "changed", inserted: [{ start: 8, end: 13 }], deleted: [{ at: 8, text: "Owning spec" }], substitutions: [] }),
              row(owners, ["Focus", "focus.spec.md"]),
              row(["Name", "Since"], ["Layout", "2026"], { kind: "removed" }),
            ]),
          ],
        },
      }),
      false,
    )
    expect(
      view.contract[0]!.entries[0]!.blocks.map(block =>
        block.kind === "paragraph" ? ["paragraph", block.paragraph.text] : ["table", block.head?.text ?? null, block.rows.map(it => [it.text, it.change?.kind ?? null])],
      ),
    ).toEqual([
      ["paragraph", "The limits of the window."],
      [
        "table",
        null,
        [
          ["Limit | Value\nTool windows | 40", "added"],
          ["Limit | Value\nProjects | 600", "changed"],
        ],
      ],
      ["paragraph", "The owners of the window."],
      ["table", "Topic | Owner", [["Topic | Owner\nFocus | focus.spec.md", "added"]]],
      ["table", null, [["Name | Since\nLayout | 2026", "removed"]]],
    ])
  })

  it("passes paragraph forms and resolves substitution marks to their original words", () => {
    const paths = ["spec/a.spec.md", "spec/b.spec.md", "spec/c.spec.md", "spec/d.spec.md", "spec/e.spec.md"]
    const form: ProseParagraph["form"] = { kind: "code", language: "sh" }
    const paragraph: ProseParagraph = {
      text: "brd",
      form,
      code: [],
      links: [],
      change: { kind: "substituted", substitutions: [{ start: 0, end: 3, substitution: 0 }] },
      excerpt: null,
    }
    const view = presentPrologue(
      plan([], [], {
        files: paths.map(path => ({ path, oldPath: null }) as FileFacts),
        brief: {
          issue: null,
          substitutions: [{ from: "bd", to: "brd", places: 3, paths }],
          contract: [contract("spec/a.spec.md", null, [paragraph])],
        },
      }),
      false,
    )
    const presented = paragraphsOf(view.contract[0]!.entries[0]!)[0]!
    expect([presented.form, presented.substitutions]).toEqual([form, [{ start: 0, end: 3, from: "bd" }]])
    expect(view.substitutions).toEqual([
      {
        key: "0:bd:brd",
        from: "bd",
        to: "brd",
        summary: "3 places in 5 files",
        files: paths.map(path => ({ path, name: path.slice(path.lastIndexOf("/") + 1), place: { path, side: "new", line: null, name: null } })),
        allFiles: paths.join("\n"),
      },
    ])
  })

  it("words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves", () => {
    const test = (name: string, line: number, way: StepTest["way"], reaches: string[]): StepTest => ({
      unit: "u:test",
      path: "test/RuleTest.kt",
      line,
      name,
      depth: 1,
      way,
      reaches,
    })
    const unreached = (name: string) => ({ name, path: "src/Frame.kt", line: 4 })
    // the rule three tests of the change reach, each by its way, a use no test of the change reaches
    const ruleTests: Partial<ReviewStep["tests"]> = {
      proof: [
        {
          path: "test/RuleTest.kt",
          tests: [test("allows", 3, "calls", ["allowed"]), test("focuses", 12, "testCode", ["Welcome.focus"]), test("keeps", 9, "endToEnd", ["allowed", "Welcome.focus"])],
        },
      ],
      reached: 2,
      unreached: [unreached("show")],
    }
    // a fact as the page words it: its words, its names, the count of the rest
    const worded = (words: string, list: FactList) => [words, ...list.places.map(it => it.label), ...(list.more > 0 ? [`and ${list.more} more`] : [])].join(" ")
    const unproven = (facts: StepFacts) => (facts.unproven === null ? null : worded("No test of the change reaches", facts.unproven.names))
    const view = presentPrologue(
      plan(
        [
          step("a", [commit("the rule"), commit("the rule again", "followsUp")], [["u:rule", "src/Rule.kt"]], ruleTests),
          step("b", [commit("the paint")], [["u:paint", "src/Frame.kt"]], { unreached: ["paint", "create", "run", "open"].map(unreached) }),
          step("c", [commit("the story")], [["u:spec", "spec/a.spec.md"]], { scenarios: ["tests/restart.json"], unreached: [unreached("load")] }),
          step("d", [commit("the try")], []),
          step("w", [commit("the wip", "noStory")], [["u:wip", "src/Wip.kt"]]),
        ],
        [
          story(
            "the rule",
            [
              paragraph("One rule decides."),
              paragraph("The product decision is to keep the layout."),
              label("Look closely", "lookClosely"),
              paragraph("Is a stored layout enough?"),
              paragraph("A layout of more tool windows may need more."),
            ],
            [
              [0, 1, false],
              [1, 2, false],
              [2, 5, false],
            ],
          ),
          story(
            "the rule again",
            [paragraph("Renamed."), label("Look closely", "lookClosely"), paragraph("Is the new name clear?")],
            [
              [0, 1, false],
              [1, 3, false],
            ],
          ),
          // a body of a few words is the story of its commit, but no lead of the step it is in
          story("the wip", [paragraph("Fixes it.")]),
        ],
        {
          brief: {
            issue: null,
            substitutions: [],
            contract: [
              contract("spec/a.spec.md", "Startup", ["The window keeps its layout."], ["tests/restart.json"]),
              contract("spec/a.spec.md", "Restart", ["The restart keeps it."]),
              contract("docs/b.md", "Why", ["Because."]),
            ],
          },
        },
      ),
      false,
    )
    expect([view.issue, view.title, view.untold, view.source]).toEqual([null, "feature/layout", null, null])
    // the steps are told: What Changes is quoted in the opening of the step that changes it, the map names it in one line
    expect(view.contract).toEqual([])
    expect(view.map.contract).toEqual({ newSpecs: [], changed: "2 specs changed" })
    expect(
      view.steps.map(it => [
        it.number,
        it.title,
        it.lead === null ? null : it.lead.block.block,
        it.facts.line,
        it.facts.tests.map(fact => worded(fact.words, fact.names)),
        unproven(it.facts),
      ]),
    ).toEqual([
      // the contents: the files of the tests of the change, the rest in its opening
      [1, plainTitle("the rule"), paragraph("One rule decides."), null, ["Tested by RuleTest.kt"], null],
      [2, plainTitle("the paint"), null, null, [], "No test of the change reaches paint create run open"],
      [3, plainTitle("the story"), null, "Proven by restart.json", ["No test of the change reaches load"], null],
      [4, plainTitle("the try"), null, "Nothing of it lands: a later commit took it out", [], null],
      [5, plainTitle("the wip"), null, null, [], null],
    ])
    // the opening, a line each: the files of the tests of the change, each at its first test with its tests and their ways
    // in its hover; the code no test of the change reaches at the line of its name
    const rule = step("a", [commit("the rule")], [["u:rule", "src/Rule.kt"]], ruleTests)
    const opening = presentStepFacts(rule, plan([rule], []))
    expect(opening.line).toBeNull()
    expect(opening.tests.map(fact => [fact.key, worded(fact.words, fact.names)])).toEqual([
      ["change", "Tested by RuleTest.kt"],
      ["none", "No test of the change reaches show"],
    ])
    expect(opening.tests[0]!.names.places.map(it => [it.place, it.detail])).toEqual([
      [
        { path: "test/RuleTest.kt", side: "new", line: 3, name: null },
        "allows: calls allowed\nfocuses: reaches Welcome.focus through the test code\nkeeps: reaches allowed and Welcome.focus end to end",
      ],
    ])
    expect(opening.tests[1]!.names.places.map(it => it.place)).toEqual([{ path: "src/Frame.kt", side: "new", line: 4, name: "show" }])
    // a step accio answered nothing complete of: no name, no warning
    const paint = step("b", [commit("the paint")], [["u:paint", "src/Frame.kt"]])
    expect([presentStepFacts(paint, plan([paint], [])).tests, presentStepFacts(paint, plan([paint], [])).unproven]).toEqual([[], null])
    // the step of the tests names the steps they prove, each showing that step
    const proving = step("t", [commit("the tests")], [["u:test", "test/RuleTest.kt"]], {}, { proves: [0] })
    expect(presentStepFacts(proving, plan([rule, proving], [])).proves).toEqual([{ key: "a", step: "step 1", stepId: "a" }])
    // the new API in the contents a list as every list: four whole, from five the first three and the count; the opening
    // names it all
    const added = (name: string): ReviewPlan["api"][number] => ({
      path: `src/a/${name}.kt`,
      line: 3,
      name,
      key: `a.${name}`,
      kind: "added",
      widened: false,
      changes: [],
      before: null,
      after: null,
      usages: null,
    })
    const apiOf = (count: number) => {
      const api = ["Cache", "Reader", "Store", "Index", "Lock"].slice(0, count).map(added)
      const adding = { ...rule, newApi: api.map(it => it.key) }
      const of = (form: "contents" | "opening") => {
        const { places, more } = presentStepFacts(adding, { ...plan([adding], []), api }, form).newApi
        return [places.map(it => it.label), more]
      }
      return [of("contents"), of("opening")]
    }
    expect(apiOf(4)).toEqual([
      [["Cache", "Reader", "Store", "Index"], 0],
      [["Cache", "Reader", "Store", "Index"], 0],
    ])
    expect(apiOf(5)).toEqual([
      [["Cache", "Reader", "Store"], 2],
      [["Cache", "Reader", "Store", "Index", "Lock"], 0],
    ])
    // the opening names nine whole; of a longer list its first eight, the rest kept for "and N more"; the contents none
    const many = (count: number, form: "contents" | "opening") => {
      const api = Array.from({ length: count }, (_, index) => added(`Codec${index}`))
      const adding = { ...rule, newApi: api.map(it => it.key) }
      const { places, more, rest } = presentStepFacts(adding, { ...plan([adding], []), api }, form).newApi
      return [places.length, more, rest?.map(it => it.label) ?? null]
    }
    expect([many(9, "opening"), many(12, "opening"), many(12, "contents")]).toEqual([
      [9, 0, null],
      [8, 4, ["Codec8", "Codec9", "Codec10", "Codec11"]],
      [3, 9, null],
    ])
    // two new declarations of one name, each led by its type: a package leads none
    const field = (key: string): ReviewPlan["api"][number] => ({ ...added("rightEdge"), key })
    const twins = [field("t.Event.rightEdge"), field("t.Update.rightEdge"), field("t.rightEdge"), added("Cache")]
    const twinStep = { ...rule, newApi: twins.map(it => it.key) }
    expect(presentStepFacts(twinStep, { ...plan([twinStep], []), api: twins }).newApi.places.map(it => it.label)).toEqual([
      "Event.rightEdge",
      "Update.rightEdge",
      "rightEdge",
      "Cache",
    ])
    // a declaration opened wider is no new API: it opens wider, at its declaration, and breaks no use
    const widened = { ...added("mediaDuration"), widened: true, before: shownForm("internal fun mediaDuration(millis: Long): String") }
    const widening = { ...rule, newApi: [widened.key, "a.Cache"] }
    const wider = presentStepFacts(widening, { ...plan([widening], []), api: [widened, added("Cache")] })
    expect([wider.newApi.places.map(it => it.label), wider.opensWider.places]).toEqual([
      ["Cache"],
      [{ key: "a.mediaDuration:src/a/mediaDuration.kt:3", label: "mediaDuration", place: { path: "src/a/mediaDuration.kt", side: "new", line: 3, name: "mediaDuration" } }],
    ])
    // the new declarations of a module the change adds are counted under its label, their names in its hover, a click
    // at the first: its whole surface is new; a new declaration of a module that was there stays named
    const media = (name: string): ReviewPlan["api"][number] => ({ ...added(name), path: `media/src/${name}.kt` })
    const surface = [media("Player"), media("Decoder"), added("Cache")]
    const adding = { ...rule, newApi: surface.map(it => it.key) }
    const modular = presentStepFacts(adding, {
      ...plan([adding], []),
      api: surface,
      modules: [moduleShape("media", "view.gui.media", { state: "added" }), moduleShape("src", "view.gui")],
      files: [fileFacts("media/src/Player.kt", 0), fileFacts("media/src/Decoder.kt", 0), fileFacts("src/a/Cache.kt", 1)],
    })
    expect([modular.newApi.places.map(it => it.label), modular.newModules.map(it => [it.module.label, it.module.detail, it.module.place.path, it.count])]).toEqual([
      ["Cache"],
      [["view.gui.media", "Player, Decoder", "media/src/Player.kt", "2 declarations"]],
    ])
    // a step none of whose code lands names the commit that took it out, which shows its step
    const tookOut = presentStepFacts(
      step("d", [commit("the try")], [], {}, { takenOutBy: "the paint" }),
      plan([step("b", [commit("the paint")], [["u:paint", "src/Frame.kt"]])], []),
    )
    // by its step, the title of the commit in the hover: a subject inside the sentence would not read
    expect([tookOut.line, tookOut.takenOutBy]).toEqual([null, { words: "step 1", title: "the paint", step: "b" }])
    // a step whose units another step's section shows lands them there: it says what it does to them and where
    const unitOf = (id: string, kind: ReviewUnit["kind"], name: string, test = false): ReviewUnit => ({
      id,
      kind,
      name,
      key: name,
      signature: "",
      api: false,
      test,
      line: 3,
      slices: [{ path: "src/Manager.kt", oldPath: null, whole: false, blocks: [], fingerprint: id }],
      fingerprint: id,
    })
    const shown = presentStepFacts(
      step(
        "h",
        [commit("drop the hooks")],
        [],
        {},
        {
          shownElsewhere: [
            { unit: "u:hooks", step: 1 },
            { unit: "u:flags", step: 1 },
            { unit: "u:passes", step: 0 },
            { unit: "u:opens", step: 0 },
            { unit: "u:focuses", step: 0 },
          ],
        },
      ),
      plan([step("h", [commit("drop the hooks")], []), step("b", [commit("the paint")], [["u:paint", "src/Frame.kt"]])], [], {
        units: [
          unitOf("u:hooks", { kind: "removed" }, "hooks"),
          unitOf("u:flags", { kind: "removed" }, "flags"),
          unitOf("u:passes", { kind: "added" }, "passes"),
          unitOf("u:opens", { kind: "added" }, "the open shows the stored layout", true),
          unitOf("u:focuses", { kind: "removed" }, "the welcome focuses its project", true),
        ],
      }),
    )
    // each name at its place, a removed one before the commits; a test, named by a sentence, unnamed and counted
    const shownWords = shown.shownElsewhere.map(it => {
      const words = [...it.names.places.map(name => name.label), ...(it.tests === null ? [] : [it.tests])]
      return [`${it.verb} ${joinNames(words)}, shown in ${it.step}`, it.stepId]
    })
    expect([shown.line, shownWords]).toEqual([
      null,
      [
        ["Removes hooks and flags, shown in step 2", "b"],
        ["Adds passes and a test, shown in step 1", "h"],
        ["Removes a test, shown in step 1", "h"],
      ],
    ])
    expect(shown.shownElsewhere[0]!.names.places[0]).toEqual({
      key: "u:hooks",
      label: "hooks",
      place: { path: "src/Manager.kt", side: "old", line: 3, name: "hooks", unit: "u:hooks" },
    })
    // where the author asks the reviewer to decide, in the contents too: its question alone, the opening tells the rest
    expect(view.steps[0]!.lookClosely.map(({ part }) => [part.head?.block.type === "label" && part.head.block.text, part.blocks.map(it => it.block)])).toEqual([
      ["Look closely", [paragraph("Is a stored layout enough?")]],
      // of a round, whose story is folded in the opening
      ["Look closely", [paragraph("Is the new name clear?")]],
    ])

    // the only step what no evidence joined: no step to tell; the issue is the headline, a range without a branch by its commits
    const rest: Concern = { id: "r", name: { source: "rest" }, parts: [], paths: [], elsewhere: [], evidence: [], fingerprint: "r" }
    const untold = presentPrologue(
      plan([{ ...step("r", [commit("fix", "noStory")], [["u:a", "a.kt"]]), concern: "r" }], [], {
        branch: null,
        title: "11111111..22222222",
        concerns: [rest],
        brief: { issue: "IJAI-1", substitutions: [], contract: [] },
        steps: {
          source: { kind: "concerns", reason: "noStory" },
          told: true,
          steps: [{ ...step("r", [commit("fix", "noStory")], [["u:a", "a.kt"]]), concern: "r" }],
          coverage: { blocks: 0, inStops: 0, mechanical: 0, missing: 0, twice: 0 },
          commits: [],
        },
      }),
      false,
    )
    expect([untold.steps, untold.source, untold.issue, untold.title]).toEqual([[], null, "IJAI-1", "11111111..22222222"])
  })

  it("does not give Other changes a number in the overview", () => {
    const task: Concern = {
      id: "task",
      name: { source: "declaration", name: "aFunction" },
      parts: [],
      paths: [],
      elsewhere: [],
      evidence: [],
      fingerprint: "task",
    }
    const rest: Concern = { ...task, id: "rest", name: { source: "rest" }, fingerprint: "rest" }
    const review = plan(
      [
        { ...step("task", [commit("task")], []), concern: "task" },
        { ...step("rest", [], []), concern: "rest" },
      ],
      [],
      { concerns: [task, rest] },
    )
    const view = presentPrologue(review, false)
    expect(view.steps.map(({ number, title }) => [number, title])).toEqual([
      [1, plainTitle("aFunction")],
      [null, plainTitle("Other changes")],
    ])
    expect(presentOpenings(review).get("rest")?.number).toBeNull()
  })

  it("shows the lead of a body and its parts before the fold, the rest and every verification section folded, each named in the outline", () => {
    const body = [
      paragraph("The open keeps the stored layout."),
      paragraph("Three startup steps showed the view."),
      label("Tests", "verification"),
      list("ManagerTest", "FrameTest"),
      label("The cause"),
      paragraph("A hook nothing sets."),
      paragraph("And a second paragraph of it."),
      label("The fix"),
      paragraph("The open asks the layout."),
      label("Evidence, on the final code", "verification"),
      paragraph("All green."),
    ]
    // Rust folds after the section of The cause, and every verification section
    const view = presentStory(
      story("the open", body, [
        [0, 1, false],
        [1, 2, false],
        [2, 4, true],
        [4, 7, false],
        [7, 9, true],
        [9, 11, true],
      ]),
    )
    expect(textOf(view)).toEqual({
      lead: "The open keeps the stored layout.",
      shown: [
        ["", "Three startup steps showed the view."],
        ["The cause:", "A hook nothing sets.", "And a second paragraph of it."],
      ],
      folded: ["Tests", "The fix", "Evidence, on the final code"],
    })
    // a run of no label the fold cuts is two parts, the second folded
    expect(
      textOf(
        presentStory(
          story(
            "prose",
            [paragraph("Lead."), paragraph("Two."), paragraph("Three."), paragraph("Four.")],
            [
              [0, 1, false],
              [1, 3, false],
              [3, 4, true],
            ],
          ),
        ),
      ),
    ).toEqual({
      lead: "Lead.",
      shown: [["", "Two.", "Three."]],
      folded: [MORE],
    })
    // a body that opens with a label has no lead
    expect(textOf(presentStory(story("labelled", [label("Why"), paragraph("Because.")]))).lead).toBeNull()
    // the author's Look closely shows where it is, also after the fold
    const asks = [
      paragraph("Lead."),
      label("The cause"),
      paragraph("A hook."),
      label("Look closely", "lookClosely"),
      paragraph("Is it enough?"),
      label("Tests", "verification"),
      list("ManagerTest"),
    ]
    expect(
      textOf(
        presentStory(
          story("asks", asks, [
            [0, 1, false],
            [1, 3, true],
            [3, 5, false],
            [5, 7, true],
          ]),
        ),
      ),
    ).toEqual({ lead: "Lead.", shown: [["Look closely:", "Is it enough?"]], folded: ["The cause", "Tests"] })
  })

  it("marks what changed in a body since the review where Rust placed it: the added blocks, the removed ones before the block they stood before or at the end of their part, the subject it had", () => {
    const body = [
      paragraph("The open keeps the stored layout."),
      label("Look closely", "lookClosely"),
      paragraph("Is it enough?"),
      label("Tests", "verification"),
      list("FrameTest"),
    ]
    const commit = story("the open", body, [
      [0, 1, false],
      [1, 3, false],
      [3, 5, false],
    ])
    const removed = (text: string) => ({ blocks: [paragraph(text)], code: [] })
    const none = { blocks: [], code: [] }
    const change = {
      subjectBefore: "the open asks",
      hunks: [
        // the lead rewritten: the old one before the new one
        { start: 0, added: { blocks: [body[0]!], code: [] }, removed: removed("The open asks the layout.") },
        // a new Look closely section
        { start: 1, added: { blocks: [body[1]!, body[2]!], code: [] }, removed: none },
        // a paragraph removed from the end of Tests
        { start: 5, added: none, removed: removed("All green.") },
      ],
    }
    const view = presentStory({
      ...commit,
      change,
      parts: commit.parts.map((part, index) => ({ ...part, removals: index === 0 ? [{ hunk: 0, before: 0 }] : index === 2 ? [{ hunk: 2, before: null }] : [] })),
    })
    expect(view.subjectBefore).toBe("the open asks")
    expect(view.lead).toEqual({ index: 0, block: body[0], added: true })
    expect(view.leadRemoved).toEqual([{ key: "the open:removed:0", before: 0, blocks: [paragraph("The open asks the layout.")], code: [] }])
    const [asks, tests] = view.shown
    // a section whose label is new is new whole, its blocks marked too
    expect([asks!.added, asks!.blocks.map(it => it.added), asks!.removed]).toEqual([true, [true], undefined])
    expect([tests!.added, tests!.blocks.map(it => it.added), tests!.removed?.map(run => run.before)]).toEqual([undefined, [undefined], [null]])
    // a story whose message did not change marks nothing
    const plain = presentStory(commit)
    expect([plain.subjectBefore, plain.lead?.added, plain.leadRemoved, plain.shown.map(part => part.removed)]).toEqual([undefined, undefined, undefined, [undefined, undefined]])
  })

  it("says in the overview how the message of a rewritten or a reworded commit changed, the blocks Rust compared under its line", () => {
    const messageChange = { subjectBefore: null, hunks: [{ start: 0, added: { blocks: [paragraph("The token outlives the tab.")], code: [] }, removed: { blocks: [], code: [] } }] }
    const commits = [
      {
        id: "c1",
        reviewedId: "r1",
        subject: "Add the token",
        state: "rewritten" as const,
        changes: { newOnly: [{ path: "src/Token.kt", lines: ["+a"] }], reviewedOnly: [] },
        messageChange,
      },
      { id: "c2", reviewedId: "r2", subject: "Style", state: "reworded" as const, changes: null, messageChange },
      {
        id: "c3",
        reviewedId: "r3",
        subject: "Log out",
        state: "rewritten" as const,
        changes: { newOnly: [], reviewedOnly: [{ path: "src/Out.kt", lines: ["+b"] }] },
        messageChange: null,
      },
      { id: "c4", reviewedId: "r4", subject: "Tidy", state: "unchanged" as const, changes: null, messageChange: null },
    ]
    const since = presentPrologue(plan([], [], { interdiff: { reviewedBase: "b", reviewedTip: "r4", commits, scope: null } }), false).since!
    expect(since.summary).toBe("Since your review: 2 rewritten · 1 reworded")
    expect(since.rewritten.map(commit => [commit.text, commit.message === null ? null : commit.message.hunks.length])).toEqual([
      ["Rewritten Add the token: src/Token.kt +1 −0 · message changed", 1],
      ["Reworded Style: message changed", 1],
      ["Rewritten Log out: src/Out.kt +0 −1", null],
    ])
  })

  it("quotes What Changes in the opening of the step whose stops show its entry, each entry in one step", () => {
    const steps = [
      step("a", [commit("the rule")], [["other:spec/a.spec.md", "spec/a.spec.md"]]),
      step(
        "b",
        [commit("the doc")],
        [
          ["other:docs/b.md", "docs/b.md"],
          ["other:spec/a.spec.md", "spec/a.spec.md"],
        ],
      ),
    ]
    const openings = presentOpenings(
      plan(steps, [story("the rule", [paragraph("One rule decides.")]), story("the doc", [paragraph("The doc says why.")])], {
        files: [{ path: "spec/a.spec.md", oldPath: null } as FileFacts, { path: "docs/b.md", oldPath: null } as FileFacts],
        brief: {
          issue: null,
          substitutions: [],
          contract: [
            contract("spec/a.spec.md", "Startup", ["The window keeps its layout."], ["tests/restart.json"]),
            { ...contract("docs/b.md", "Why", ["Because."]), kind: "doc" },
          ],
        },
      }),
    )
    const quoted = (id: string) =>
      openings.get(id)!.contract.map(file => [file.path, file.kind, file.entries.map(entry => [entry.heading, paragraphsOf(entry).map(it => it.text), entry.provenBy])])
    expect(quoted("a")).toEqual([["spec/a.spec.md", "Spec rule", [["Startup", ["The window keeps its layout."], "Proven by restart.json"]]]])
    // the spec is quoted in the first step that shows it, the doc in its own
    expect(quoted("b")).toEqual([["docs/b.md", "Doc", [["Why", ["Because."], null]]]])
  })

  it("opens a step of the commits with the body of the commit that starts it, its follow-ups and the commits of no story, its facts with the new API and the new links between modules at their places", () => {
    const openings = presentOpenings(
      plan(
        [
          step(
            "a",
            [commit("the rule"), commit("the rule again", "followsUp"), commit("wip", "noStory")],
            [["u:rule", "src/a/Rule.kt"]],
            {},
            { newApi: ["a.Cache", "a.Store"], moduleEdges: [{ from: 0, to: 1, links: [0, 1] }] },
          ),
          step("no-story", [commit("tidy", "noStory")], [["u:tidy", "src/Tidy.kt"]]),
        ],
        [story("the rule", [paragraph("One rule decides.")]), story("the rule again", [paragraph("Renamed it.")])],
        {
          brief: { issue: "IJAI-1", substitutions: [], contract: [] },
          api: [
            {
              path: "src/a/Cache.kt",
              line: 3,
              name: "Cache",
              key: "a.Cache",
              kind: "added",
              widened: false,
              changes: [],
              before: null,
              after: shownForm("class Cache"),
              usages: null,
            },
            { path: "src/a/Store.kt", line: 4, name: "Store", key: "a.Store", kind: "changed", widened: false, changes: [], before: null, after: null, usages: null },
          ],
          links: [
            { from: "src/a/Reader.kt", to: "src/b/Cache.kt", symbols: [{ name: "Cache", line: 3 }], state: "added", crossModule: true, newBetweenModules: true },
            { from: "src/a/Writer.kt", to: "src/b/Cache.kt", symbols: [{ name: "Cache", line: 5 }], state: "added", crossModule: true, newBetweenModules: true },
          ],
          modules: [moduleShape("src/a", "src/a"), moduleShape("src/b", "src/b")],
        },
      ),
    )
    const rule = openings.get("a")!
    expect([rule.number, rule.title, rule.evidence, textOf(rule.story!).lead]).toEqual(["Step 1", plainTitle("the rule"), [], "One rule decides."])
    // the other commits of the step, one form each: its role muted, its title, a story to open or none
    expect(rule.commits.map(it => [it.role, it.title, it.story === null ? null : textOf(it.story).lead])).toEqual([
      ["Follows up", plainTitle("the rule again"), "Renamed it."],
      ["Without a story", plainTitle("wip"), null],
    ])
    // a changed declaration is no new API
    expect([rule.facts.line, rule.facts.newApi, rule.facts.moduleEdges.places.map(({ label, place }) => [label, place])]).toEqual([
      null,
      { places: [{ key: "a.Cache:src/a/Cache.kt:3", label: "Cache", place: { path: "src/a/Cache.kt", side: "new", line: 3, name: "Cache" } }], more: 0 },
      // a pair of modules once, its links counted, at the first use of its first link, in the file that uses
      [["src/a → src/b (2 links)", { path: "src/a/Reader.kt", side: "new", line: 3, name: "Cache" }]],
    ])
    // the contents leave the new uses between modules to the map: the opening names them
    const contents = presentPrologue(
      plan([step("a", [commit("the rule")], [["u:rule", "src/a/Rule.kt"]], {}, { moduleEdges: [{ from: 0, to: 1, links: [0] }] })], [], {
        links: [{ from: "src/a/Reader.kt", to: "src/b/Cache.kt", symbols: [{ name: "Cache", line: 3 }], state: "added", crossModule: true, newBetweenModules: true }],
        modules: [moduleShape("src/a", "src/a"), moduleShape("src/b", "src/b")],
      }),
      false,
    )
    expect(contents.steps[0]!.facts.moduleEdges.places).toEqual([])
    const untold = openings.get("no-story")!
    expect([untold.title, untold.story, untold.commits.map(it => it.title)]).toEqual([plainTitle("Changes without a story"), null, [plainTitle("tidy")]])
  })

  it("tells each part of the body of one commit where Rust places it, the label of the title of a step not again, the lead and the rest in the overview", () => {
    const concern = (id: string, detail: string): Concern => ({
      id,
      name: { source: "token", token: detail },
      parts: [],
      paths: [],
      elsewhere: [],
      evidence: [{ kind: "token", detail, joins: 1 }],
      fingerprint: id,
    })
    const one = story(
      "the open keeps the layout",
      [
        paragraph("The open keeps the stored layout."),
        paragraph("Names nothing."),
        label("The rule"),
        paragraph("allowed decides."),
        label("The paint"),
        paragraph("paint comes first."),
      ],
      [
        [0, 1, false],
        [1, 2, false],
        [2, 4, false],
        [4, 6, false],
      ],
      [{ kind: "overview" }, { kind: "overview" }, { kind: "step", step: 0 }, { kind: "step", step: 1 }],
      2,
    )
    const concerns = [concern("c1", "allowed"), concern("c2", "paint")]
    // the first task titled by its label, which its opening then tells no more
    const steps = [
      { ...step("c1", [commit("the open keeps the layout")], [["u:allowed", "src/a/Rule.kt"]]), concern: "c1", title: plainTitle("The rule") },
      { ...step("c2", [commit("the open keeps the layout")], [["u:paint", "src/a/Frame.kt"]]), concern: "c2" },
    ]
    const whole = plan(steps, [one], {
      concerns,
      brief: { issue: "IJAI-1", substitutions: [], contract: [] },
      steps: { source: { kind: "concerns", reason: "oneCommit" }, told: true, steps, coverage: { blocks: 0, inStops: 0, mechanical: 0, missing: 0, twice: 0 }, commits: [one] },
    })
    const openings = presentOpenings(whole)
    expect([openings.get("c1")!.title, textOf(openings.get("c1")!.story!)]).toEqual([plainTitle("The rule"), { lead: null, shown: [["", "allowed decides."]], folded: [] }])
    expect(textOf(openings.get("c2")!.story!)).toEqual({ lead: null, shown: [["The paint:", "paint comes first."]], folded: [] })
    // what joins the parts of a task, its names code in the line of the facts
    expect(openings.get("c1")!.evidence).toEqual([{ words: "Joined by the identifier", names: ["allowed"] }])
    // each part told at one place: the lead and what names nothing in the overview
    const prologue = presentPrologue(whole, false)
    expect(prologue.source).toBe("One commit: the steps are its tasks")
    expect(prologue.untold?.subject).toEqual(plainTitle("the open keeps the layout"))
    expect(textOf(prologue.untold!.story)).toEqual({ lead: "The open keeps the stored layout.", shown: [["", "Names nothing."]], folded: [] })
  })

  it("tells each part of the message of one commit in the step it is, a title of its first sentence read in its paragraph, a step larger, the commit no step's row", () => {
    const one = story(
      "a URL is one link",
      [
        paragraph("A TUI breaks a URL at the edge."),
        paragraph("The marker comes from the emulator. Only it knows the edge."),
        paragraph("The markers travel in `Update.edges`, the model and the state."),
        list("the event fills them", "the model keeps them"),
        label("Look closely", "lookClosely"),
        paragraph("the guards of `Joiner.join`."),
      ],
      [
        [0, 1, false],
        [1, 2, false],
        [2, 4, false],
        [4, 6, false],
      ],
      [{ kind: "overview" }, { kind: "step", step: 0 }, { kind: "step", step: 1 }, { kind: "step", step: 1 }],
    )
    const head = commit("a URL is one link")
    const steps = [
      { ...step("m1", [head], [["u:edge", "src/Grid.kt"]]), title: plainTitle("The marker comes from the emulator."), titleSource: "sentence" as const },
      { ...step("m2", [head], [["u:edges", "src/Model.kt"]]), title: plainTitle("Update.edges"), titleSource: "name" as const },
    ]
    const whole = plan(steps, [one], {
      steps: { source: { kind: "message" }, told: true, steps, coverage: { blocks: 0, inStops: 0, mechanical: 0, missing: 0, twice: 0 }, commits: [one] },
    })
    const openings = presentOpenings(whole)
    // the first sentence titles the step: its paragraph is the lead of the opening, the title only for a screen reader
    const m1 = openings.get("m1")!
    expect([m1.title, m1.titleInBody, textOf(m1.story!)]).toEqual([
      plainTitle("The marker comes from the emulator."),
      true,
      { lead: "The marker comes from the emulator. Only it knows the edge.", shown: [], folded: [] },
    ])
    // a title of a code name is code, above the paragraph and the list under it, the Look closely after them
    const m2 = openings.get("m2")!
    expect([m2.title, m2.code, m2.titleInBody, m2.commits]).toEqual([plainTitle("Update.edges"), true, false, []])
    expect(textOf(m2.story!).shown.map(([head]) => head)).toEqual(["", "Look closely:"])
    // the overview: why the steps are what they are, their titles, the Look closely in the contents of its step
    const prologue = presentPrologue(whole, false)
    expect(prologue.source).toBe("One commit: the steps are the parts of its message")
    expect(prologue.steps.map(it => [it.title, it.code, it.lookClosely.length])).toEqual([
      [plainTitle("The marker comes from the emulator."), false, 0],
      [plainTitle("Update.edges"), true, 1],
    ])
    expect(textOf(prologue.untold!.story)).toEqual({ lead: "A TUI breaks a URL at the edge.", shown: [], folded: [] })
    // its one commit is the row of no step: the steps are its parts
    const places = placeCommits(steps, [{ commit: { id: "a URL is one link", subject: "a URL is one link" } }], "message")
    expect([places.heads.size, places.steps.get("m1")?.map(it => it.title)]).toEqual([0, [plainTitle("a URL is one link")]])
  })

  it("links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it", () => {
    const unit: ReviewUnit = {
      id: "declaration:src/a/Manager.kt:a.Manager.allowed:",
      kind: { kind: "added" },
      name: "allowed",
      key: "a.Manager.allowed",
      signature: "",
      api: false,
      test: false,
      line: 6,
      slices: [],
      fingerprint: "f",
    }
    const named: Mention = {
      block: 0,
      item: 0,
      start: 22,
      end: 39,
      text: "Manager.allowed",
      target: { kind: "unit", unit: unit.id, name: "allowed", path: "src/a/Manager.kt", side: "new", line: 6, step: 1 },
    }
    const file: Mention = { block: 2, item: 1, start: 2, end: 12, text: "Frame.kt", target: { kind: "file", path: "src/a/Frame.kt", step: null } }
    const loose: Mention = { block: 0, item: 0, start: 50, end: 58, text: "Layout", target: null }
    const rule = {
      ...story(
        "the rule",
        [paragraph("One rule decides now: `Manager.allowed` answers, by `Layout`."), label("The uses"), list("one", "- `Frame.kt` asks it")],
        [
          [0, 1, false],
          [1, 3, false],
        ],
      ),
      mentions: [named, loose, file],
    }
    const steps = [step("a", [commit("the hooks")], [["u:hooks", "src/a/Manager.kt"]]), step("b", [commit("the rule")], [[unit.id, "src/a/Manager.kt"]])]
    const whole = plan(steps, [story("the hooks", [paragraph("No hooks.")]), rule], { units: [unit] })

    // a declaration by the heading of its unit, its line and its step, shown with its name; a file by its path; a name
    // that resolves nothing has no place
    const view = presentStory(rule, undefined, mentionWords(whole))
    expect(view.mentions.map(it => [it.text, it.description, it.place])).toEqual([
      [
        "Manager.allowed",
        "New · Manager.kt:6 · in step 2",
        { path: "src/a/Manager.kt", side: "new", line: 6, name: "allowed", unit: "declaration:src/a/Manager.kt:a.Manager.allowed:" },
      ],
      ["Layout", null, null],
      ["Frame.kt", "src/a/Frame.kt", { path: "src/a/Frame.kt", side: "new", line: null, name: null }],
    ])

    // a declaration of a new file, whose unit is the file, by the name the body writes, the name Rust resolved selected
    // once shown
    const fresh: ReviewUnit = { ...unit, id: "file:src/a/LayoutStore.kt", kind: { kind: "newFile" }, name: "LayoutStore.kt", key: null, line: null }
    expect(
      mentionWords({ units: [fresh] })({ kind: "unit", unit: fresh.id, name: "read", path: "src/a/LayoutStore.kt", side: "new", line: 4, step: 4 }, "LayoutStore.read"),
    ).toEqual({
      place: { path: "src/a/LayoutStore.kt", side: "new", line: 4, name: "read", unit: "file:src/a/LayoutStore.kt" },
      description: "New file · LayoutStore.kt:4 · in step 5",
      // the card of its hover: what it is and where, never the name again; its step, which is not the one read
      card: { status: "New file", tone: "success", location: "LayoutStore.kt:4", step: "in step 5" },
    })
    // read in its own step, the card names no step
    expect(
      mentionWords({ units: [fresh] }, 4)({ kind: "unit", unit: fresh.id, name: "read", path: "src/a/LayoutStore.kt", side: "new", line: 4, step: 4 }, "LayoutStore.read").card
        .step,
    ).toBeNull()

    // the lines Rust found naming the stops of each section, worded where they are told, one per block
    const section = (key: string, story: NarrativeSection["story"]): NarrativeSection => ({
      key,
      path: "src/a/Manager.kt",
      units: { marks: new Map(), elsewhere: new Map() },
      unitIds: new Set(),
      story,
      targets: new Map(),
      heading: null,
      groupName: null,
      group: null,
      concern: "b",
      concernHeading: null,
      folded: null,
      ghost: null,
      part: null,
    })
    const marks = storyMarks(whole, [
      section("file:src/a/Manager.kt", []),
      section("file:src/a/Manager.kt#1", [
        { commit: "the rule", block: 0, item: 0, step: 1 },
        { commit: "the rule", block: 0, item: 0, step: 1 },
      ]),
      section("file:src/a/Frame.kt", [{ commit: "the rule", block: 2, item: 1, step: 1 }]),
      section("file:src/a/Old.kt", [{ commit: "the hooks", block: 0, item: 0, step: null }]),
    ])
    expect([...marks].map(([key, list]) => [key, list.map(mark => [mark.commit, mark.block, mark.step, mark.title, mark.preview])])).toEqual([
      ["file:src/a/Manager.kt#1", [["the rule", 0, "b", "Step 2 · the rule", "One rule decides now: `Manager.allowed` answers, by `Layout`."]]],
      ["file:src/a/Frame.kt", [["the rule", 2, "b", "Step 2 · the rule", "- `Frame.kt` asks it"]]],
      ["file:src/a/Old.kt", [["the hooks", 0, null, "Overview", "No hooks."]]],
    ])
    // the heading of its card: the place, then the label of the step, an identifier set as code
    expect(marks.get("file:src/a/Frame.kt")!.map(mark => [mark.place, mark.label, mark.code])).toEqual([["Step 2", "the rule", false]])
    // the quote of a mark links the code names of its line, worded as the story words them
    expect(marks.get("file:src/a/Manager.kt#1")![0]!.mentions.map(it => [it.text, it.description])).toEqual([
      ["Manager.allowed", "New · Manager.kt:6 · in step 2"],
      ["Layout", null],
    ])
  })

  it("quotes a cell of a table as the line of a mark, the header row first, then row by row", () => {
    const table: CommitBodyBlock = { type: "table", header: ["Part", "Where"], align: ["none", "right"], rows: [["the frame", "`Frame.kt`"]] }
    const sized = story("the sizes", [paragraph("The sizes."), table])
    const whole = plan([step("a", [commit("the sizes")], [["u:frame", "src/a/Frame.kt"]])], [sized])
    const section: NarrativeSection = {
      key: "file:src/a/Frame.kt",
      path: "src/a/Frame.kt",
      units: { marks: new Map(), elsewhere: new Map() },
      unitIds: new Set(),
      story: [{ commit: "the sizes", block: 1, item: 3, step: 0 }],
      targets: new Map(),
      heading: null,
      groupName: null,
      group: null,
      concern: "a",
      concernHeading: null,
      folded: null,
      ghost: null,
      part: null,
    }
    expect(
      storyMarks(whole, [section])
        .get(section.key)!
        .map(mark => [mark.block, mark.preview]),
    ).toEqual([[1, "`Frame.kt`"]])
    // a table is a block of its part, read by its rows
    const parted = presentStory(
      story(
        "the sizes",
        [paragraph("The sizes."), table],
        [
          [0, 1, false],
          [1, 2, false],
        ],
      ),
    )
    expect(textOf(parted)).toEqual({ lead: "The sizes.", shown: [["", "Part | Where / the frame | `Frame.kt`"]], folded: [] })
  })

  it("opens each group of a module of a large step above its first card: its name, its state, what enters a new module, its tests, its new API by name, its uses between modules by their direction, what its module registers and its texts; the opening of the step keeps none of them", () => {
    const api = (name: string, path: string): ReviewPlan["api"][number] => ({
      path,
      line: 3,
      name,
      key: `k.${name}`,
      kind: "added",
      widened: false,
      changes: [],
      before: null,
      after: null,
      usages: null,
    })
    const link = (symbol: string, line: number): ReviewPlan["links"][number] => ({
      from: "media/src/Player.kt",
      to: "gui/src/Kind.kt",
      symbols: [{ name: symbol, line }],
      state: "added",
      crossModule: true,
      newBetweenModules: true,
    })
    // the tests of the group of the new module, as Rust gives them: a test reaches the player, none the flag of shared/code
    const tested: StepTests = {
      answered: true,
      proof: [
        {
          path: "media/testSrc/PlayerTest.kt",
          tests: [{ unit: "u:test", path: "media/testSrc/PlayerTest.kt", line: 6, name: "plays", depth: 1, way: "calls", reaches: ["Player"] }],
        },
      ],
      reached: 1,
      unreached: [{ name: "Flag", path: "shared/src/Flag.kt", line: 3 }],
      scenarios: [],
    }
    const media = step(
      "media",
      [commit("play the media")],
      [
        ["u:spec", "spec/media.spec.md"],
        ["u:kind", "gui/src/Kind.kt"],
        ["u:player", "media/src/Player.kt"],
        ["u:flag", "shared/src/Flag.kt"],
        ["u:test", "media/testSrc/PlayerTest.kt"],
        ["mechanical:build", "media/BUILD.bazel"],
      ],
      {},
      {
        newApi: ["k.VIDEO", "k.Player"],
        moduleEdges: [{ from: 1, to: 0, links: [0, 1] }],
        groups: [
          stepGroup("contract", null, [0, 1, 1]),
          stepGroup("module", 0, [1, 2, 2]),
          stepGroup("module", 1, [2, 4, 5], { also: [2], tested }),
          stepGroup("mechanical", null, [5, 6, 6]),
        ],
      },
    )
    const whole = plan([media], [], {
      api: [api("VIDEO", "gui/src/Kind.kt"), api("Player", "media/src/Player.kt")],
      links: [link("Kind", 4), link("VIDEO", 5)],
      modules: [
        moduleShape("gui", "view.gui"),
        moduleShape("media", "view.gui.media", {
          state: "added",
          // what enters the new module, as accio answers it: a class its descriptor registers, one the code of gui uses
          entries: {
            complete: true,
            points: [
              {
                name: "PlayersImpl",
                qualified: "a.media.PlayersImpl",
                kind: "class",
                path: "media/src/PlayersImpl.kt",
                line: 4,
                registered: { point: "com.intellij.applicationService", of: "a.gui.Players", path: "media/resources/media.xml", line: 3 },
                usedBy: [],
              },
              {
                name: "Player",
                qualified: "a.media.Player",
                kind: "class",
                path: "media/src/Player.kt",
                line: 3,
                registered: null,
                usedBy: [
                  { path: "gui/src/Kind.kt", line: 7 },
                  { path: "gui/src/View.kt", line: 9 },
                ],
              },
            ],
          },
        }),
        moduleShape("shared", "shared/code"),
      ],
      files: [fileFacts("gui/src/Kind.kt", 0), fileFacts("media/src/Player.kt", 1), fileFacts("shared/src/Flag.kt", 2), fileFacts("media/testSrc/PlayerTest.kt", 1)],
      registrations: [
        {
          descriptor: "media/resources/media.xml",
          module: 1,
          change: "added",
          registered: {
            kind: "service",
            area: "app",
            interface: "a.gui.Players",
            implementation: "a.media.PlayersImpl",
            testImplementation: null,
            headlessImplementation: null,
            client: null,
            os: null,
            preload: null,
            overrides: false,
            open: false,
          },
          before: null,
          changed: [],
          classes: [],
        },
      ],
      texts: [{ path: "gui/resources/messages/GuiBundle.properties", key: "embed.play", change: "added", before: null, after: "Play", line: 4, module: 0 }],
    })
    const openings = presentGroupOpenings(whole)
    // the groups of the modules open, the contract and the wiring do not
    expect([...openings.keys()]).toEqual(["media:1", "media:2"])
    const labels = (list: FactList | undefined) => list?.places.map(it => it.label) ?? []
    const gui = openings.get("media:1")!
    expect([
      gui.name,
      gui.code,
      gui.state,
      gui.also,
      labels(gui.facts.newApi),
      labels(gui.facts.uses),
      labels(gui.facts.usedBy),
      gui.facts.tests,
      gui.registrations,
      gui.texts?.counts,
    ]).toEqual(["view.gui", true, null, null, ["VIDEO"], [], ["view.gui.media (2 links)"], [], [], "1 new"])
    // the new module names its new API, since the group is the module; its tests and its code none reaches, the flag of
    // the small module it holds too
    const player = openings.get("media:2")!
    // what enters it first: a registration by its point and what it is for, a use by the file it is in
    expect([labels(player.facts.entries), player.facts.entries?.places[1]?.detail, player.facts.entriesNote, gui.facts.entries]).toEqual([
      ["PlayersImpl (applicationService of Players)", "Player (used in Kind.kt and 1 more)"],
      "a.media.Player\ngui/src/Kind.kt:7\ngui/src/View.kt:9",
      null,
      undefined,
    ])
    // a complete answer of none says so; a partial one says it may lack some
    const noteOf = (entries: ModuleShape["entries"]) =>
      presentGroupOpenings({ ...whole, modules: whole.modules.map((module, place) => (place === 1 ? { ...module, entries } : module)) }).get("media:2")!.facts.entriesNote
    expect([noteOf({ complete: true, points: [] }), noteOf({ complete: false, points: [] }), noteOf(null)]).toEqual([
      "Nothing outside the module uses it",
      "accio stopped before it asked every declaration",
      undefined,
    ])
    expect([
      player.name,
      player.state,
      player.also,
      labels(player.facts.newApi),
      player.facts.newModules,
      labels(player.facts.uses),
      player.facts.tests.map(it => [it.words, labels(it.names)]),
      player.registrations.map(it => `${it.sign} ${it.kind} ${it.name}`),
      player.texts,
    ]).toEqual([
      "view.gui.media",
      "New module",
      "also shared/code",
      ["Player"],
      [],
      ["view.gui (2 links)"],
      [
        ["Tested by", ["PlayerTest.kt"]],
        ["No test of the change reaches", ["Flag"]],
      ],
      ["+ applicationService Players → PlayersImpl"],
      null,
    ])
    // the opening of the step leaves to them what they say of its code
    const facts = presentOpenings(whole).get("media")!.facts
    expect([facts.tests, facts.newApi.places, facts.newModules, facts.opensWider.places, facts.moduleEdges.places, facts.unproven]).toEqual([[], [], [], [], [], null])
  })
})
