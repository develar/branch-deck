import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/test-fixtures"
import { syncFromMenu } from "./sync-helpers"
import type { TestRepositoryBuilder } from "./test-repository"

/**
 * How the push helpers name the branches of `prefix` on the remote: the start of the name of each kind, and of the
 * name a helper gives it (a virtual branch by its name, the stack branch of a whole stack as `stack:<root>`, an
 * unapplied branch as `unapplied:<name>`)
 */
function kinds(prefix: string): [branch: string, name: string][] {
  return [
    [`${prefix}/virtual/`, ""],
    [`${prefix}/stack/`, "stack:"],
    [`${prefix}/unapplied/`, "unapplied:"],
  ]
}

/** The virtual branch of `name` under the branch prefix of the templates, `user-name/virtual/<name>` */
export function virtualBranch(name: string, prefix = "user-name"): string {
  return `${prefix}/virtual/${name}`
}

/** How a push helper names the stack branch of the whole stack of `root`, `user-name/stack/<root>`: `stack:<root>` */
export function stackBranchOf(root: string): string {
  return `stack:${root}`
}

/**
 * The branch a push helper names `name`: `stack:<root>` (`stackBranchOf`) the stack branch of the whole stack of
 * `root`, `user-name/stack/<root>`, `unapplied:<name>` the unapplied branch `user-name/unapplied/<name>`, any other
 * name its virtual branch
 */
export function branchOf(name: string, prefix = "user-name"): string {
  const kind = kinds(prefix).find(([, start]) => start !== "" && name.startsWith(start))
  return kind === undefined ? virtualBranch(name, prefix) : kind[0] + name.slice(kind[1].length)
}

/** The name a push helper gives the branch `branch` of the remote (`branchOf`), null for one not of `prefix` */
function nameOf(branch: string, prefix: string): string | null {
  const kind = kinds(prefix).find(([start]) => branch.startsWith(start))
  return kind === undefined ? null : kind[1] + branch.slice(kind[0].length)
}

/** The names (`branchOf`) of the branches of `prefix` among `branches`, sorted */
function namesOf(branches: Iterable<string>, prefix: string): string[] {
  return [...branches].flatMap(branch => nameOf(branch, prefix) ?? []).sort()
}

/** The branches of `prefix` the remote of `repo` has, sorted, named as `branchOf` names them: what the pushes left there */
export async function pushedBranches(repo: TestRepositoryBuilder, prefix = "user-name"): Promise<string[]> {
  return namesOf(Object.keys(await repo.getRemoteBranches()), prefix)
}

/**
 * The pushes of the app the remote of `repo` took, in order, each the branches of `prefix` it moved, sorted, named as
 * `branchOf` names them: one entry for the one atomic push of a stack, none for a push the remote refused or one that
 * found the remote moved and pushed nothing
 */
export async function remotePushes(repo: TestRepositoryBuilder, prefix = "user-name"): Promise<string[][]> {
  return (await repo.getRemotePushes()).map(push => namesOf(Object.keys(push), prefix))
}

/**
 * Waits until the remote of `repo` has the branches `names` (`branchOf`) at the commits their local branches are at,
 * and its log has the push of the app that moved each there: the push of each reached the remote and ended, so
 * `remotePushes` has it. The hook that logs a push runs after its refs moved (docs/design/e2e-testing.md#the-remote-of-a-repository)
 */
export async function expectPushedToRemote(repo: TestRepositoryBuilder, names: readonly string[], prefix = "user-name"): Promise<void> {
  const tips = Object.fromEntries(await Promise.all(names.map(async name => [name, await repo.revision(branchOf(name, prefix))] as const)))
  await expect
    .poll(async () => {
      const [remote, pushes] = await Promise.all([repo.getRemoteBranches(), repo.getRemotePushes()])
      return Object.fromEntries(
        names.map(name => {
          const branch = branchOf(name, prefix)
          const logged = pushes.some(push => push[branch] === tips[name])
          return [name, logged ? (remote[branch] ?? null) : `${remote[branch] ?? null}, no push logged`]
        }),
      )
    })
    .toEqual(tips)
}

/**
 * The branches `names` (`branchOf`) pushed before the test, behind the app's back (`pushInTerminal`), each as the sync
 * built it, without its newest `lacking` commits, or (`earlierBuild`) as an earlier build of the same commits; then a
 * sync, so the rows show them on the remote. The commits pushed, by name
 */
export async function pushedBefore(
  page: Page,
  repo: TestRepositoryBuilder,
  names: readonly string[],
  { earlierBuild = false, lacking = 0 }: { earlierBuild?: boolean; lacking?: number } = {},
): Promise<Record<string, string>> {
  const pushed: Record<string, string> = {}
  for (const name of names) {
    pushed[name] = await repo.pushInTerminal(branchOf(name), { revision: `${branchOf(name)}~${lacking}`, earlierBuild })
  }
  await syncFromMenu(page)
  return pushed
}

/**
 * Holds the pushes at the remote of `repo` from now on (the held answer `push`): a push reaches the remote, which has
 * its pack, and moves no ref until the release, so a test sees the push running. `arrived` waits until one reached
 * the remote and waits there; `release` lets it and the later ones through
 */
export async function holdRemotePushes(repo: TestRepositoryBuilder): Promise<{ arrived: () => Promise<void>; release: () => Promise<void> }> {
  await repo.holdRemoteAnswers({ hold: ["push"] })
  return {
    arrived: () => expect.poll(() => repo.getArrivedAnswers()).toContain("push"),
    release: () => repo.holdRemoteAnswers({ release: ["push"] }),
  }
}

/** The branches of the `stacked_branches` template */
export const STACKED_BRANCHES = ["auth", "login", "session", "oauth", "docs"]

/** The branches of the `stack_island` template: its stack from `air`, and `auth`, `login` and `docs` */
export const STACK_ISLAND_BRANCHES = ["air", "bazel", "bridge", "runtime", "side-fix", "deploy", "side-test", "auth", "login", "docs"]

/**
 * A fix of someone else on the remote branch of each branch of `authors` (by name, its author: "Ann <ann@example.com>"),
 * "Fix <name>" pushed behind the app's back onto master, then fetched: the push of each drops it. The commits, by name
 */
export async function fixesOfOthersOnRemote(repo: TestRepositoryBuilder, authors: Readonly<Record<string, string>>): Promise<Record<string, string>> {
  const theirs: Record<string, string> = {}
  for (const [name, author] of Object.entries(authors)) {
    await repo.pushInTerminal(virtualBranch(name), { revision: "origin/master" })
    theirs[name] = await repo.commitOnRemote(virtualBranch(name), { subject: `Fix ${name}`, author, timestamp: 1704139200, files: { [`${name}-fix.txt`]: "fix\n" } })
  }
  await repo.fetchInTerminal()
  return theirs
}

/**
 * A push that drops remote commits asks first: the click on its button (the toast's action, P in the preview) opens its
 * description with the dropped commits and a destructive Force Push button; this clicks that button, which pushes.
 */
export async function confirmForcePush(page: Page, label: string): Promise<void> {
  // the description of another push may still be closing
  const description = page.getByTestId("push-description").filter({ has: page.getByTestId("push-confirm") })
  await expect(description.getByTestId("dropped-commits")).toBeVisible()
  const confirm = description.getByTestId("push-confirm")
  await expect(confirm).toHaveText(label)
  await confirm.click()
}

/**
 * The toast of a push of `branchName`, by its title: "Pushed branch feature-auth", or "Force pushed branch feature-auth"
 * (the words of its Activity entry)
 */
export function pushedToast(page: Page, branchName: string): Locator {
  const name = branchName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
  return page.locator("[data-slot=toast]").filter({ has: page.locator("[data-slot=toast-title]", { hasText: new RegExp(`^(Force p|P)ushed branch ${name}$`) }) })
}

/** Waits for the toast of a push of `branchName`; with `description`, its detail ("3 commits · created the remote branch") */
export async function expectPushedToast(page: Page, branchName: string, description?: string): Promise<void> {
  const toast = pushedToast(page, branchName)
  await expect(toast).toBeVisible()
  if (description !== undefined) {
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText(description)
  }
}
