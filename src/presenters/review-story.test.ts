import { describe, expect, it } from "vite-plus/test"
import type { CommitBodyBlock, Concern, ContractEntry, Mention, ReviewPlan, ReviewStep, ReviewUnit, StoryCommit, TestRef } from "@/lib/bindings"
import { MORE } from "@/presenters/commit-body"
import { mentionWords, presentOpenings, presentPrologue, presentStepFacts, presentStory, storyMarks } from "@/presenters/review-story"
import type { StoryView } from "@/presenters/review-story"
import type { NarrativeSection } from "@/presenters/review-units"
import { reviewPlan, reviewStep, stepCommit } from "@/test-support/review-step"

const commit = stepCommit
const step = reviewStep

const paragraph = (text: string): CommitBodyBlock => ({ type: "paragraph", lines: [text] })
const label = (text: string, kind: "plain" | "verification" | "lookClosely" = "plain"): CommitBodyBlock => ({ type: "label", text, kind, runIn: false })
const list = (...items: string[]): CommitBodyBlock => ({ type: "list", ordered: false, start: 1, items })

/** A part of a body as Rust cuts it (`sync_utils::commit_body`): its first block, the block after its last, folded or not */
type Part = readonly [start: number, end: number, folded: boolean]

/**
 * The story of the commit of `subject`: its blocks, the lead when it opens with prose, and its `parts` written out as
 * Rust cuts that body (the TS tests never encode the grammar); one part of every block by default, a body of one block
 * or of one section
 */
function story(subject: string, blocks: CommitBodyBlock[], parts: readonly Part[] = [[0, blocks.length, false]], toldIn: (number | null)[] = []): StoryCommit {
  const lead = blocks[0]?.type === "paragraph"
  return { id: subject, title: subject, body: { blocks, trailers: [], lead, parts: parts.map(([start, end, folded]) => ({ start, end, folded })), code: [] }, mentions: [], toldIn }
}

function plan(steps: ReviewStep[], commits: StoryCommit[], more: Partial<ReviewPlan> = {}): ReviewPlan {
  const base = reviewPlan()
  return reviewPlan({ steps: { ...base.steps, steps, commits }, ...more })
}

/** The text of the blocks of a part: a paragraph its lines, a list its items */
function textOf(view: StoryView): { lead: string | null; shown: string[][]; folded: string[] } {
  const words = (block: CommitBodyBlock) => (block.type === "paragraph" ? block.lines.join(" ") : block.type === "list" ? block.items.join(" | ") : block.type)
  return {
    lead: view.lead === null ? null : words(view.lead.block),
    shown: view.shown.map(part => [part.head?.block.type === "label" ? `${part.head.block.text}:` : "", ...part.blocks.map(it => words(it.block))]),
    folded: view.folded.map(it => it.name),
  }
}

function contract(path: string, heading: string, added: string[], provenBy: string[] = []): ContractEntry {
  return {
    kind: "spec",
    unit: `other:${path}`,
    path,
    block: 0,
    heading,
    newSpec: null,
    deleted: false,
    added,
    addedCode: added.map(() => []),
    removed: [],
    removedCode: [],
    provenBy,
  }
}

describe("the document of a review", () => {
  it("words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves", () => {
    const test = (name: string, unit: string | null, line = 3): TestRef => ({ unit, path: "test/RuleTest.kt", line, name, depth: 1, via: [] })
    const unreached = (name: string, within: number | null = null) => ({ name, within })
    const view = presentPrologue(
      plan(
        [
          step("a", [commit("the rule"), commit("the rule again", "followsUp")], [["u:rule", "src/Rule.kt"]], {
            reached: [
              { name: "allowed", tests: [test("allows", "u:test"), test("keeps", null, 9)] },
              { name: "focus", tests: [test("focuses", null, 12), test("keeps", null, 9)] },
            ],
            unreached: [unreached("show")],
          }),
          step("b", [commit("the paint")], [["u:paint", "src/Frame.kt"]], {
            unreached: ["paint", "create", "run", "open"].map(name => unreached(name)).concat(unreached("close", 3)),
          }),
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
    expect(view.contract.map(file => [file.path, file.kind, file.entries.map(entry => [entry.heading, entry.paragraphs.map(it => it.text), entry.provenBy])])).toEqual([
      [
        "spec/a.spec.md",
        "Spec rule",
        [
          ["Startup", ["The window keeps its layout."], "Proven by restart.json"],
          ["Restart", ["The restart keeps it."], null],
        ],
      ],
      ["docs/b.md", "Spec rule", [["Why", ["Because."], null]]],
    ])
    expect(view.steps.map(it => [it.number, it.title, it.lead === null ? null : it.lead.block.block, it.facts.line, it.facts.unproven])).toEqual([
      [1, "the rule", paragraph("One rule decides."), "Tests of this change reach allowed · 2 tests it did not touch reach allowed, focus · No test reaches show", null],
      [2, "the paint", null, null, "No test reaches paint, create, run, open +1 within 3 calls"],
      [3, "the story", null, "Proven by restart.json · No test reaches load", null],
      [4, "the try", null, "Nothing of it lands: a later commit took it out", null],
      [5, "the wip", null, null, null],
    ])
    // a step none of whose code lands names the commit that took it out, which shows its step
    const tookOut = presentStepFacts(
      step("d", [commit("the try")], [], {}, { takenOutBy: "the paint" }),
      plan([step("b", [commit("the paint")], [["u:paint", "src/Frame.kt"]])], []),
    )
    // by its step, the title of the commit in the hover: a subject inside the sentence would not read
    expect([tookOut.line, tookOut.takenOutBy]).toEqual([null, { words: "step 1", title: "the paint", step: "b" }])
    // a step whose units another step's section shows lands them there: it says what it does to them and where
    const unitOf = (id: string, kind: ReviewUnit["kind"], name: string): ReviewUnit => ({
      id,
      kind,
      name,
      key: name,
      signature: "",
      api: false,
      test: false,
      line: 3,
      slices: [],
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
          ],
        },
      ),
      plan([step("h", [commit("drop the hooks")], []), step("b", [commit("the paint")], [["u:paint", "src/Frame.kt"]])], [], {
        units: [unitOf("u:hooks", { kind: "removed" }, "hooks"), unitOf("u:flags", { kind: "removed" }, "flags"), unitOf("u:passes", { kind: "added" }, "passes")],
      }),
    )
    expect([shown.line, shown.shownElsewhere.map(it => [`${it.words} ${it.step}`, it.stepId])]).toEqual([
      null,
      [
        ["Removes hooks, flags, shown in step 2", "b"],
        ["Adds passes, shown in step 1", "h"],
      ],
    ])
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
        brief: { issue: "IJAI-1", contract: [] },
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

  it("opens a step of the commits with the body of the commit that starts it, its follow-ups and the commits of no story, its facts with the new API and the new links between modules at their places", () => {
    const openings = presentOpenings(
      plan(
        [
          step(
            "a",
            [commit("the rule"), commit("the rule again", "followsUp"), commit("wip", "noStory")],
            [["u:rule", "src/a/Rule.kt"]],
            {},
            { newApi: ["a.Cache", "a.Store"], moduleLinks: [0] },
          ),
          step("no-story", [commit("tidy", "noStory")], [["u:tidy", "src/Tidy.kt"]]),
        ],
        [story("the rule", [paragraph("One rule decides.")]), story("the rule again", [paragraph("Renamed it.")])],
        {
          brief: { issue: "IJAI-1", contract: [] },
          api: [
            { path: "src/a/Cache.kt", line: 3, name: "Cache", key: "a.Cache", kind: "added", changes: [], before: null, after: "class Cache", usages: null },
            { path: "src/a/Store.kt", line: 4, name: "Store", key: "a.Store", kind: "changed", changes: [], before: null, after: null, usages: null },
          ],
          links: [{ from: "src/a/Reader.kt", to: "src/b/Cache.kt", symbols: [{ name: "Cache", line: 3 }], state: "added", crossModule: true, newBetweenModules: true }],
        },
      ),
    )
    const rule = openings.get("a")!
    expect([rule.number, rule.title, rule.evidence, textOf(rule.story!).lead]).toEqual(["Step 1", "the rule", [], "One rule decides."])
    // the other commits of the step, one form each: its role muted, its title, a story to open or none
    expect(rule.commits.map(it => [it.role, it.title, it.story === null ? null : textOf(it.story).lead])).toEqual([
      ["Follows up", "the rule again", "Renamed it."],
      ["Without a story", "wip", null],
    ])
    // a changed declaration is no new API
    expect([rule.facts.line, rule.facts.newApi, rule.facts.moduleLinks.map(({ label, place }) => [label, place])]).toEqual([
      null,
      [{ key: "a.Cache:src/a/Cache.kt:3", label: "Cache", place: { path: "src/a/Cache.kt", side: "new", line: 3, name: "Cache" } }],
      // at its first use, in the file that uses
      [["Reader.kt → Cache.kt", { path: "src/a/Reader.kt", side: "new", line: 3, name: "Cache" }]],
    ])
    const untold = openings.get("no-story")!
    expect([untold.title, untold.story, untold.commits.map(it => it.title)]).toEqual(["Changes without a story", null, ["tidy"]])
  })

  it("tells the body of one commit in the openings of the concerns its sections name, the label a title once, its lead and the rest in the overview", () => {
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
      [null, null, 0, 0, 1, 1],
    )
    const concerns = [concern("c1", "allowed"), concern("c2", "paint")]
    // the first task titled by its label, which its opening then tells no more
    const steps = [
      { ...step("c1", [commit("the open keeps the layout")], [["u:allowed", "src/a/Rule.kt"]]), concern: "c1", title: "The rule", toldAsTitle: 2 },
      { ...step("c2", [commit("the open keeps the layout")], [["u:paint", "src/a/Frame.kt"]]), concern: "c2" },
    ]
    const whole = plan(steps, [one], {
      concerns,
      brief: { issue: "IJAI-1", contract: [] },
      steps: { source: { kind: "concerns", reason: "oneCommit" }, told: true, steps, coverage: { blocks: 0, inStops: 0, mechanical: 0, missing: 0, twice: 0 }, commits: [one] },
    })
    const openings = presentOpenings(whole)
    expect([openings.get("c1")!.title, textOf(openings.get("c1")!.story!)]).toEqual(["The rule", { lead: null, shown: [["", "allowed decides."]], folded: [] }])
    expect(textOf(openings.get("c2")!.story!)).toEqual({ lead: null, shown: [["The paint:", "paint comes first."]], folded: [] })
    // what joins the parts of a task, its names code in the line of the facts
    expect(openings.get("c1")!.evidence).toEqual([{ words: "Joined by the identifier", names: ["allowed"] }])
    // each part told at one place: the lead and what names nothing in the overview
    const prologue = presentPrologue(whole, false)
    expect(prologue.source).toBe("One commit: the steps are its tasks")
    expect(prologue.untold?.subject).toBe("the open keeps the layout")
    expect(textOf(prologue.untold!.story)).toEqual({ lead: "The open keeps the stored layout.", shown: [["", "Names nothing."]], folded: [] })
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
      ["Manager.allowed", "New · Manager.kt:6 · in step 2", { path: "src/a/Manager.kt", side: "new", line: 6, name: "allowed" }],
      ["Layout", null, null],
      ["Frame.kt", "src/a/Frame.kt", { path: "src/a/Frame.kt", side: "new", line: null, name: null }],
    ])

    // a declaration of a new file, whose unit is the file, by the name the body writes, the name Rust resolved selected
    // once shown
    const fresh: ReviewUnit = { ...unit, id: "file:src/a/LayoutStore.kt", kind: { kind: "newFile" }, name: "LayoutStore.kt", key: null, line: null }
    expect(
      mentionWords({ units: [fresh] })({ kind: "unit", unit: fresh.id, name: "read", path: "src/a/LayoutStore.kt", side: "new", line: 4, step: 4 }, "LayoutStore.read"),
    ).toEqual({
      place: { path: "src/a/LayoutStore.kt", side: "new", line: 4, name: "read" },
      description: "New file · LayoutStore.kt:4 · in step 5",
      // the card of its hover: what it is and where, never the name again; its step, which is not the one read
      card: { status: "New file", tone: "success", location: "LayoutStore.kt:4", step: "in step 5" },
    })
    // read in its own step, the card names no step
    expect(
      mentionWords({ units: [fresh] }, 4)({ kind: "unit", unit: fresh.id, name: "read", path: "src/a/LayoutStore.kt", side: "new", line: 4, step: 4 }, "LayoutStore.read").card
        .step,
    ).toBeNull()

    // the section of the path in the step of the target, the one that holds the unit, not another of the same heading;
    // a file in any step
    const section = (key: string, path: string, concern: string, marks: [number, string][] = [], unitIds: string[] = []): NarrativeSection => ({
      key,
      path,
      units: { marks: new Map(marks), elsewhere: new Map() },
      unitIds: new Set(unitIds),
      targets: new Map(),
      heading: null,
      concern,
      concernHeading: null,
      folded: null,
      ghost: null,
    })
    const marks = storyMarks(whole, [
      section("file:src/a/Manager.kt", "src/a/Manager.kt", "a", [[0, "hooks — removed"]], ["u:hooks"]),
      section("file:src/a/Manager.kt#1", "src/a/Manager.kt", "b", [[0, "allowed — new"]], [unit.id]),
      section("file:src/a/Manager.kt#2", "src/a/Manager.kt", "b", [[1, "allowed — new"]], [`${unit.id}:id:String`]),
      section("file:src/a/Frame.kt", "src/a/Frame.kt", "b"),
    ])
    expect([...marks].map(([key, list]) => [key, list.map(mark => [mark.commit, mark.block, mark.step, mark.title, mark.preview])])).toEqual([
      ["file:src/a/Manager.kt#1", [["the rule", 0, "b", "Step 2 · the rule", "One rule decides now: `Manager.allowed` answers, by `Layout`."]]],
      ["file:src/a/Frame.kt", [["the rule", 2, "b", "Step 2 · the rule", "- `Frame.kt` asks it"]]],
    ])
    // the heading of its card: the place, then the label of the step, an identifier set as code
    expect(marks.get("file:src/a/Frame.kt")!.map(mark => [mark.place, mark.label, mark.code])).toEqual([["Step 2", "the rule", false]])
    // the quote of a mark links the code names of its line, worded as the story words them
    expect(marks.get("file:src/a/Manager.kt#1")![0]!.mentions.map(it => [it.text, it.description])).toEqual([
      ["Manager.allowed", "New · Manager.kt:6 · in step 2"],
      ["Layout", null],
    ])
  })
})
