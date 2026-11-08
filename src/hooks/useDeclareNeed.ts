import { undoRegroup } from "@/hooks/useStackActions"
import type { NeedResult, RewriteIntent } from "@/lib/bindings"
import type { RewriteDone } from "@/lib/rewrite-run"
import { runRewriteIntent, witnessed } from "@/lib/rewrite-run"
import { joinNames } from "@/lib/text"
import { showUndoToast } from "@/lib/undo-toast"
import { presentRewrite } from "@/presenters/rewrite"
import { getSelectedRepositoryPath } from "@/stores/repository"

export interface NeedOptions {
  /** The surface that started it and shows its spinner (`SelectContext.surface`) */
  surface?: string | null
}

/**
 * Declares that `needs` land before `branchName` (spec/branch/declared-needs.spec.md): the rewrite `need` of the engine,
 * no rewrite of a commit, one commit of `refs/branch-deck/needs` under the repository lock, then the sync that names
 * the need on the row, then the toast with Undo, which moves the store back to the commit it was written on. A refusal
 * of the core (an unknown branch, a need of itself) is a toast and false: the form keeps what was typed
 */
export function declareNeed(branchName: string, needs: readonly string[], options: NeedOptions = {}): Promise<boolean> {
  return runNeed({ rewrite: "need", branchName, needs: [...needs] }, `${joinNames(needs)} ${needs.length === 1 ? "lands" : "land"} before ${branchName}`, options)
}

/** Takes the needs of `needs` of `branchName` back in one write (the rewrite `unneed`), with Undo */
export function removeNeed(branchName: string, needs: readonly string[], options: NeedOptions = {}): Promise<boolean> {
  return runNeed({ rewrite: "unneed", branchName, needs: [...needs] }, `Removed the need of ${branchName} for ${joinNames(needs)}`, options)
}

async function runNeed(intent: Omit<Extract<RewriteIntent, { rewrite: "need" | "unneed" }>, "witness">, title: string, options: NeedOptions): Promise<boolean> {
  // the repository of the click: the write and its Undo go there, whichever the main window shows when they run
  const repositoryPath = getSelectedRepositoryPath()
  const rewrite = witnessed(repositoryPath, intent)
  const done = await runRewriteIntent({ surface: options.surface, repositoryPath }, rewrite)
  const result = neededOf(done)
  if (done === null || result === null) {
    return false
  }
  if (result.changed) {
    const { phrase } = presentRewrite(rewrite)
    const shown = intent.rewrite === "need" ? `Declared that ${title}` : title
    showUndoToast(repositoryPath, shown, () => undoRegroup(repositoryPath, done.undo?.token, `Cannot undo ${phrase}`, `Undoing ${phrase}`), undefined, done.outcome)
  }
  return true
}

/** What a need or its removal answered; null for another answer */
function neededOf(done: RewriteDone | null): NeedResult | null {
  return done?.result.rewrite === "needed" ? done.result.answer : null
}
