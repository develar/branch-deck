import { describe, expect, it } from "vite-plus/test"
import type { Concern, ReviewPlan, ReviewStep, ReviewUnit, StepCommit, StepGroup } from "@/lib/bindings"
import { plainTitle } from "@/lib/commit"
import { headingText } from "@/lib/diff/rows"
import {
  commitTitleOf,
  formatCommitRole,
  formatStepDivider,
  formatStepNumber,
  groupHeadings,
  placeCommits,
  presentChapters,
  presentOwnerMarks,
  WITHOUT_A_STORY,
  withCommitTitles,
} from "@/presenters/review-steps"
import { presentNarrative } from "@/presenters/review-units"
import { fileFacts, moduleShape, reviewPlan, reviewStep, stepCommit, stepGroup } from "@/test-support/review-step"

const commit = stepCommit
const step = reviewStep

/** A block of `src/Manager.kt`, which the units of the joins share */
const block = { index: 0, oldStart: 4, oldEnd: 6, newStart: 4, newEnd: 9, additions: 5, deletions: 2 }

/** A declaration of `src/Manager.kt` whose name is on line 5, with `blocks` of its own */
function unit(id: string, kind: ReviewUnit["kind"], name: string, blocks: (typeof block)[]): ReviewUnit {
  return {
    id,
    kind,
    name,
    key: `a.Manager.${name}`,
    signature: "",
    api: true,
    test: false,
    line: 5,
    slices: [{ path: "src/Manager.kt", oldPath: null, whole: false, blocks, fingerprint: id }],
    fingerprint: id,
  }
}

function plan(steps: ReviewStep[], concerns: Concern[] = []): Pick<ReviewPlan, "steps" | "concerns"> {
  return {
    steps: { source: { kind: "commits" }, told: true, steps, coverage: { blocks: 0, inStops: 0, mechanical: 0, missing: 0, twice: 0 }, commits: [] },
    concerns,
  }
}

describe("review steps", () => {
  it("keeps the prefix Rust split off the title of a commit apart, the label of the step the title as one line", () => {
    const title = { prefix: "cleanup platform:", text: "JnaLoader adds System32 once" }
    const [chapter] = presentChapters(plan([step("a", [{ id: "a", title, role: "starts" }], [["u:jna", "src/Jna.kt"]], {}, { title })]))
    expect([chapter!.title, chapter!.label, chapter!.heading]).toEqual([
      title,
      "cleanup platform: JnaLoader adds System32 once",
      "1 · cleanup platform: JnaLoader adds System32 once",
    ])
  })

  it("names a step by the title Rust gives it, in the case its author wrote it, a step of no story by what it is", () => {
    const chapters = presentChapters(
      plan([
        step("a", [commit("the rule")], [["u:rule", "src/Rule.kt"]], {}, { title: plainTitle("the rule") }),
        step("no-story", [commit("wip", "noStory")], [["u:wip", "src/Wip.kt"]]),
      ]),
    )
    expect(chapters.map(chapter => [chapter.number, chapter.label, chapter.heading, chapter.reference, chapter.parts.length])).toEqual([
      [1, "the rule", "1 · the rule", "step 1", 1],
      [2, WITHOUT_A_STORY, `2 · ${WITHOUT_A_STORY}`, "step 2", 1],
    ])
    // the divider above the first section of each in the diffs: its number muted, then its title; the hover of the map
    expect(chapters.map(chapter => [formatStepNumber(chapter.number), chapter.label])).toEqual([
      ["Step 1", "the rule"],
      ["Step 2", WITHOUT_A_STORY],
    ])
    expect(chapters.map(chapter => formatStepDivider(chapter))).toEqual(["Step 1 · the rule", `Step 2 · ${WITHOUT_A_STORY}`])
  })

  it("names a step of a concern by the label of the section of the body it tells, else by its concern as code, keeps its main rows, and tells no step of a story Rust tells in none", () => {
    const concern: Concern = {
      id: "c",
      name: { source: "token", token: "proxy" },
      parts: [{ unit: "u:a", path: "a.json", blocks: null }],
      paths: ["a.json"],
      elsewhere: ["a.json"],
      evidence: [{ kind: "token", detail: "ij-proxy", joins: 2 }],
      fingerprint: "f",
    }
    const of = plan([{ ...step("c", [commit("one")], [["u:a", "a.json"]]), concern: "c" }], [concern])
    const [chapter] = presentChapters(of)
    // a told step by its number in the tree and after a row "also in" it, as the overview tells it
    expect([chapter!.label, chapter!.heading, chapter!.reference, chapter!.hover, chapter!.elsewhere, formatStepDivider(chapter!)]).toEqual([
      "proxy",
      "1 · proxy",
      "step 1",
      "Joined by the identifier ij-proxy",
      ["a.json"],
      "Step 1 · proxy",
    ])
    // its name is an identifier, set as code; a story Rust tells in no steps (one task that covers the range,
    // `ReviewSteps.told`) tells no chapter, and numbers none
    expect([chapter!.told, chapter!.code]).toEqual([true, true])
    expect(presentChapters({ ...of, steps: { ...of.steps, told: false } }).map(it => [it.told, it.heading, it.reference])).toEqual([[false, "proxy", "proxy"]])

    // beside another task, each a step; the label of the section of the body told in a step is its title, as the author
    // named the task, and is no code
    const theme: Concern = { ...concern, id: "t", name: { source: "token", token: "accent" }, paths: ["b.json"], elsewhere: [], fingerprint: "t" }
    const two = presentChapters(
      plan(
        [
          { ...step("c", [commit("one")], [["u:a", "a.json"]]), concern: "c", title: plainTitle("The proxy") },
          { ...step("t", [commit("one")], [["u:b", "b.json"]]), concern: "t" },
        ],
        [concern, theme],
      ),
    )
    expect(two.map(it => [it.label, it.told, it.code])).toEqual([
      ["The proxy", true, false],
      ["accent", true, true],
    ])
  })

  it("words Other changes in the hover of the map of the files by its name alone, as it has no number of a step", () => {
    const concern = (id: string, name: Concern["name"], path: string): Concern => ({
      id,
      name,
      parts: [{ unit: `u:${id}`, path, blocks: null }],
      paths: [path],
      elsewhere: [],
      evidence: [],
      fingerprint: id,
    })
    const chapters = presentChapters(
      plan(
        [
          { ...step("c", [commit("one")], [["u:c", "a.json"]]), concern: "c" },
          { ...step("r", [], [["u:r", "App.kt"]]), concern: "r" },
        ],
        [concern("c", { source: "token", token: "proxy" }, "a.json"), concern("r", { source: "rest" }, "App.kt")],
      ),
    )
    expect(chapters.map(chapter => [chapter.told, formatStepDivider(chapter)])).toEqual([
      [true, "Step 1 · proxy"],
      [false, "Other changes"],
    ])
  })

  it("sets names with spaces as phrases, not code", () => {
    const phrase: Concern = {
      id: "phrase",
      name: { source: "declaration", name: "a b c" },
      parts: [{ unit: "u:phrase", path: "src/Api.kt", blocks: null }],
      paths: ["src/Api.kt"],
      elsewhere: [],
      evidence: [],
      fingerprint: "phrase",
    }
    const identifier: Concern = { ...phrase, id: "identifier", name: { source: "declaration", name: "isMacBinary" }, fingerprint: "identifier" }
    const chapters = presentChapters(
      plan(
        [
          { ...step("phrase", [commit("one")], [["u:phrase", "src/Api.kt"]]), concern: "phrase" },
          { ...step("identifier", [commit("one")], [["u:identifier", "src/Other.kt"]]), concern: "identifier" },
        ],
        [phrase, identifier],
      ),
    )
    expect(chapters.map(chapter => [chapter.label, chapter.code])).toEqual([
      ["a b c", false],
      ["isMacBinary", true],
    ])

    const message = plan([step("message", [commit("one")], [], {}, { title: plainTitle("a b c"), titleSource: "name" })])
    const messagePlan: Pick<ReviewPlan, "steps" | "concerns"> = { ...message, steps: { ...message.steps, source: { kind: "message" } } }
    expect(presentChapters(messagePlan)[0]?.code).toBe(false)
  })

  it("counts the files of the chosen owner and all files each step shows", () => {
    const chapters = presentChapters(
      plan([
        step(
          "a",
          [commit("the rule")],
          [
            ["u:rule", "src/Rule.kt"],
            ["u:rule2", "src/Rule.kt"],
            ["u:style", "ui/style.css"],
          ],
        ),
        step("b", [commit("the paint")], [["u:paint", "src/Frame.kt"]]),
        step("c", [commit("the look")], [["u:look", "ui/look.css"]]),
      ]),
    )
    const marks = presentOwnerMarks(chapters, new Set(["ui/style.css", "ui/look.css"]))
    expect([...marks]).toEqual([
      ["a", { shown: 1, total: 2 }],
      ["b", { shown: 0, total: 1 }],
      ["c", { shown: 1, total: 1 }],
    ])
    expect([...presentOwnerMarks(chapters, new Set()).values()].map(({ shown }) => shown)).toEqual([0, 0, 0])
  })

  it("places each commit of the range in the step it tells or follows up, the one that starts a step of the commits as its step, a commit of the concerns once, the rest in Other commits", () => {
    // the range lists full ids, newest first; the plan names them as its input did, maybe short
    const item = (id: string, subject: string) => ({ key: `key-${id}`, commit: { id, subject } })
    const items = [item("e5e5e5e5", "unrelated"), item("d4d4d4d4", "wip"), item("c3c3c3c3", "the rule again"), item("b2b2b2b2", "the paint"), item("a1a1a1a1", "the rule")]
    // Rust names a commit of a step by its title
    const short = (id: string, role: StepCommit["role"]): StepCommit => ({
      id: id.slice(0, 4),
      title: plainTitle(items.find(it => it.commit.id === id)?.commit.subject ?? id),
      role,
    })
    const told = placeCommits(
      [step("a1a1", [short("a1a1a1a1", "starts"), short("c3c3c3c3", "followsUp"), short("d4d4d4d4", "noStory")], []), step("b2b2", [short("b2b2b2b2", "starts")], [])],
      items,
    )
    // a step of the commits is the commit that starts it; it lists the others, each by its title
    expect([...told.heads].map(([id, it]) => [id, it.id, it.subject, it.item?.key])).toEqual([
      ["a1a1", "a1a1a1a1", "the rule", "key-a1a1a1a1"],
      ["b2b2", "b2b2b2b2", "the paint", "key-b2b2b2b2"],
    ])
    expect([...told.steps].map(([id, commits]) => [id, commits.map(it => [it.id, it.subject, formatCommitRole(it.role), it.item?.key])])).toEqual([
      [
        "a1a1",
        [
          ["c3c3c3c3", "the rule again", "follows up", "key-c3c3c3c3"],
          ["d4d4d4d4", "wip", "no story", "key-d4d4d4d4"],
        ],
      ],
    ])
    expect(told.other.map(it => [it.id, it.role])).toEqual([["e5e5e5e5", null]])
    // a commit of a step by the title Rust gives it, its whole subject in its hover; a commit of no step by its subject
    const keyed = placeCommits(
      [
        step(
          "a1a1",
          [
            { id: "a1a1", title: plainTitle("the rule"), role: "starts" },
            { id: "c3c3", title: plainTitle("the rule again"), role: "followsUp" },
          ],
          [],
        ),
      ],
      [item("c3c3c3c3", "IJAI-1: the rule again"), item("a1a1a1a1", "IJAI-1 the rule"), item("e5e5e5e5", "IJAI-1 unrelated")],
    )
    expect([keyed.heads.get("a1a1")?.title, keyed.steps.get("a1a1")?.map(it => [it.title, it.subject]), keyed.other.map(it => it.title)]).toEqual([
      plainTitle("the rule"),
      [[plainTitle("the rule again"), "IJAI-1: the rule again"]],
      [plainTitle("IJAI-1 unrelated")],
    ])
    // the concerns list a commit in each concern of its lines, the most lines first: it is listed in the one that lists it
    // first, the first of equal ones; a commit of the step no item names keeps the step's id and subject
    const concerns = placeCommits(
      [
        step("proxy", [short("b2b2b2b2", "starts"), short("a1a1a1a1", "starts")], []),
        step("grep", [short("a1a1a1a1", "starts"), short("b2b2b2b2", "starts")], []),
        step("tools", [short("c3c3c3c3", "starts"), short("f6f6f6f6", "noStory")], []),
        step("look", [short("c3c3c3c3", "starts")], []),
      ].map(it => ({ ...it, concern: it.id })),
      items,
    )
    expect([...concerns.steps].map(([id, commits]) => [id, commits.map(it => [it.id, it.item === undefined])])).toEqual([
      ["proxy", [["b2b2b2b2", false]]],
      ["grep", [["a1a1a1a1", false]]],
      [
        "tools",
        [
          ["c3c3c3c3", false],
          ["f6f6", true],
        ],
      ],
    ])
    expect(concerns.other.map(it => it.id)).toEqual(["e5e5e5e5", "d4d4d4d4"])
    // a step of the concerns is no commit: each lists its commits, by their titles
    expect([concerns.heads.size, concerns.steps.get("proxy")?.[0]?.title]).toEqual([0, plainTitle("the paint")])
  })

  it("joins a declaration without a block of its own to the heading of the block Rust names, marked with its step where it is another's", () => {
    const units = [unit("u:hooks", { kind: "removed" }, "hooks", []), unit("u:allowed", { kind: "added" }, "allowed", [block]), unit("u:passes", { kind: "added" }, "passes", [])]
    // the step that dropped the hooks has no stop: the stop of `allowed` shows them, and `passes` its own step wrote
    const allowed = step("b", [commit("the rule")], [["u:allowed", "src/Manager.kt"]])
    const chapters = presentChapters(
      plan([
        step("a", [commit("drop the hooks")], [], {}, { shownElsewhere: [{ unit: "u:hooks", step: 1 }] }),
        {
          ...allowed,
          stops: allowed.stops.map(stop => ({
            ...stop,
            joined: [
              { unit: "u:hooks", block: 0, step: 0 },
              { unit: "u:passes", block: 0, step: 1 },
            ],
          })),
        },
      ]),
    )
    const sections = presentNarrative(units, [{ path: "src/Manager.kt", blocks: [block] }], [], [], chapters)
    expect(
      sections.map(section => [
        section.key,
        section.concernHeading,
        [...(section.units?.marks ?? [])].map(([block, heading]) => [block, headingText(heading)]),
        [...(section.units?.elsewhere ?? [])],
        [...section.unitIds],
      ]),
    ).toEqual([["file:src/Manager.kt", "2 · the rule", [[0, "allowed — new · hooks — removed in step 1 · passes — new"]], [], ["u:allowed", "u:hooks", "u:passes"]]])
  })
})

describe("the units Rust joined in the steps of the concerns", () => {
  // the removed hooks come first in the story, their line in the block of `allowed`
  const units = [unit("u:hooks", { kind: "removed" }, "hooks", []), unit("u:allowed", { kind: "added" }, "allowed", [block])]
  const concern = (id: string, token: string): Concern => ({
    id,
    name: { source: "token", token },
    parts: [],
    paths: ["src/Manager.kt"],
    elsewhere: [],
    evidence: [{ kind: "token", detail: token, joins: 2 }],
    fingerprint: id,
  })
  const mark = { commit: "c1", block: 0, item: 0, step: null }
  const stop = (unit: string, joined: ReviewStep["stops"][number]["joined"], marks: ReviewStep["stops"][number]["marks"] = []) => ({
    unit,
    path: "src/Manager.kt",
    blocks: null,
    uses: null,
    also: [],
    concern: null,
    joined,
    marks,
  })
  const sectionsOf = (told: boolean, steps: ReviewStep[], concerns: Concern[]) => {
    const of = plan(steps, concerns)
    const chapters = presentChapters({ ...of, steps: { ...of.steps, source: { kind: "concerns", reason: "oneCommit" }, told } })
    return presentNarrative(units, [{ path: "src/Manager.kt", blocks: [block] }], [], [], chapters).map(section => [
      [...(section.units?.marks ?? [])].map(([block, heading]) => [block, headingText(heading)]),
      [...section.unitIds],
      section.story,
    ])
  }

  it("words a joined unit after the heading of its block and keeps the marks of the stop in a story whose steps are not told", () => {
    const steps = [{ ...step("c", [commit("one")], []), concern: "c", stops: [stop("u:allowed", [{ unit: "u:hooks", block: 0, step: 0 }], [mark])] }]
    expect(sectionsOf(false, steps, [concern("c", "layout")])).toEqual([[[[0, "allowed — new · hooks — removed"]], ["u:allowed", "u:hooks"], [mark]]])
  })

  it("marks a joined unit of another told step of a concern by the number of that step", () => {
    const steps = [
      { ...step("h", [commit("one")], [], {}, { shownElsewhere: [{ unit: "u:hooks", step: 1 }] }), concern: "h" },
      { ...step("l", [commit("one")], []), concern: "l", stops: [stop("u:allowed", [{ unit: "u:hooks", block: 0, step: 0 }])] },
    ]
    expect(sectionsOf(true, steps, [concern("h", "hooks"), concern("l", "layout")])).toEqual([[[[0, "allowed — new · hooks — removed in step 1"]], ["u:allowed", "u:hooks"], []]])
  })
})

describe("withCommitTitles", () => {
  it("keeps the titles Rust gives the commits of every plan told, by a short id or a full one, and the same map when none is new", () => {
    const first = withCommitTitles(new Map(), [reviewStep("a", [{ id: "1111111111", title: plainTitle("open takes a mode"), role: "starts" }], [])])
    expect(commitTitleOf(first, "1111111")).toBe("open takes a mode")
    // the plan of one commit alone keeps the titles of the others
    const second = withCommitTitles(first, [reviewStep("b", [{ id: "2222222222", title: plainTitle("the rule names its flag again"), role: "starts" }], [])])
    expect(commitTitleOf(second, "1111111111")).toBe("open takes a mode")
    expect(commitTitleOf(second, "2222222222")).toBe("the rule names its flag again")
    expect(withCommitTitles(second, [reviewStep("b", [{ id: "2222222222", title: plainTitle("the rule names its flag again"), role: "starts" }], [])])).toBe(second)
    expect(commitTitleOf(second, "3333333")).toBeUndefined()
  })
})

describe("the groups of a large step", () => {
  it("words the divider of each group and of its tests: the contract, a module by its label and state with the small modules it holds, a folder; the mechanical group none", () => {
    const stops: [string, string][] = [
      ["other:spec/player.spec.md", "spec/player.spec.md"],
      ["new:gui/src/Embed.kt", "gui/src/Embed.kt"],
      ["new:gui/media/src/Player.kt", "gui/media/src/Player.kt"],
      ["new:shared/src/Flag.kt", "shared/src/Flag.kt"],
      ["new:gui/media/testSrc/PlayerTest.kt", "gui/media/testSrc/PlayerTest.kt"],
      ["mechanical:buildModel", "gui/media/BUILD.bazel"],
    ]
    const groups: StepGroup[] = [
      stepGroup("contract", null, [0, 1, 1]),
      stepGroup("module", 1, [1, 2, 2]),
      stepGroup("module", 0, [2, 4, 5], { also: [2] }),
      stepGroup("mechanical", null, [5, 6, 6]),
    ]
    const step = reviewStep("a", [stepCommit("Play a video")], stops, {}, { groups })
    const modules = [moduleShape("gui/media", "gui/media", { state: "added" }), moduleShape("gui", "gui"), moduleShape("shared", "shared")]
    expect([...groupHeadings(step, modules)]).toEqual([
      [0, "Contract · 1 file"],
      [1, "gui · 1 file"],
      [2, "gui/media · New module · 2 files · also shared"],
      [4, "gui/media · Tests · 1 file"],
    ])
    // the chapter carries each divider on the part that starts its group
    const [chapter] = presentChapters({ steps: { ...reviewPlan().steps, steps: [step] }, concerns: [], modules })
    expect(chapter!.parts.map(part => part.groupHeading)).toEqual([
      "Contract · 1 file",
      "gui · 1 file",
      "gui/media · New module · 2 files · also shared",
      null,
      "gui/media · Tests · 1 file",
      null,
    ])
  })

  it("names the mechanical group Wiring while it holds the build model alone, else Mechanical, its hover saying what such a group holds: moved declarations too", () => {
    const stops: [string, string][] = [
      ["new:gui/src/Embed.kt", "gui/src/Embed.kt"],
      ["mechanical:buildModel", "gui/BUILD.bazel"],
      ["mechanical:moved", "gui/src/Store.kt"],
    ]
    const modules = [moduleShape("gui", "gui")]
    const files = [fileFacts("gui/src/Embed.kt", 0), { ...fileFacts("gui/BUILD.bazel", 0), mechanical: "buildModel" as const }, fileFacts("gui/src/Store.kt", 0)]
    const groupsOf = (end: number) => {
      const groups: StepGroup[] = [stepGroup("module", 0, [0, 1, 1]), stepGroup("mechanical", null, [1, end, end])]
      const step = reviewStep("a", [stepCommit("Embed a player")], stops.slice(0, end), {}, { groups })
      const [chapter] = presentChapters({ steps: { ...reviewPlan().steps, steps: [step] }, concerns: [], modules, files })
      return chapter!.groups.map(group => [group.name, group.hover, group.paths])
    }
    expect(groupsOf(2)[1]).toEqual(["Wiring", "The build files and the lists of modules of the step", ["gui/BUILD.bazel"]])
    // a declaration that moved is of a unit, never the kind of its file: the group is no wiring with it
    expect(groupsOf(3)[1]).toEqual([
      "Mechanical",
      "The mechanical changes of the step: the build model, imports alone, whitespace, moved declarations, renames, generated files",
      ["gui/BUILD.bazel", "gui/src/Store.kt"],
    ])
  })
})
