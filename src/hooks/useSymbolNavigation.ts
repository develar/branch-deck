import { useEffect, useState } from "react"
import type { DeclarationPlace, DiffViewFile } from "@/components/diff/DiffView"
import { commands } from "@/lib/bindings"
import type { DeclarationSite, DeclarationTarget, DiffRevisions, FindDeclarationParams, UsageTarget } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import type { CommandResult } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { NavigationGate } from "@/lib/navigation-gate"
import { showToast } from "@/lib/toast-message"
import { getDiffRepositoryPath } from "@/stores/repository"

/** What a name leads to, for the user to choose one (`SymbolChooser`): its declarations, or its usages */
export type SymbolChoice = {
  name: string
  at: { x: number; y: number }
  /** Shows a target: in the view, or a file outside the review in a view of its own */
  show: (target: DeclarationTarget) => void
  /** What had the focus when the choice was asked for: Escape gives it back */
  returnFocus: HTMLElement | null
} & ({ kind: "declarations"; targets: readonly DeclarationTarget[] } | { kind: "usages"; usages: readonly UsageTarget[]; moreOutside: boolean; indexError: string | null })

/** The place of a target in the view */
export function placeOf(target: DeclarationTarget): DeclarationPlace {
  return { path: target.path, side: target.side, line: target.line, column: target.column, endColumn: target.endColumn }
}

/** The element with the focus, for a choice to give it back */
function focused(): HTMLElement | null {
  return document.activeElement instanceof HTMLElement ? document.activeElement : null
}

/** A name in a diff, as the view hands it: where it is, where to show a choice, how to show a place of the view */
type SymbolAction = (site: DeclarationSite, at: { x: number; y: number }, reveal: (place: DeclarationPlace) => boolean, from: DeclarationPlace | null) => void

/**
 * Go to Declaration and Find Usages in a review (spec/commit/symbol-navigation.spec.md): the backend finds the
 * declarations or the usages of the name in the files of the review, then in the index of the repository. One
 * declaration is shown at once (a file outside the review in a view of its own), several are offered; the usages are
 * a list; none is said in a toast. One navigation is in flight per view (`NavigationGate`): a repeat of the name joins
 * it, and an answer that arrives after another name or another subject (⌥↓) does nothing
 */
export function useSymbolNavigation(files: readonly DiffViewFile[], revisions: DiffRevisions, openFile: (target: DeclarationTarget, from: DeclarationPlace | null) => void) {
  const [choice, setChoice] = useState<SymbolChoice | null>(null)
  // one navigation in flight per view; an answer for another subject or a newer name is dropped
  const [gate] = useState(() => new NavigationGate())
  const subject = `${revisions.old ?? ""}\n${revisions.new ?? ""}`
  useEffect(() => gate.show(subject), [gate, subject])
  /** The params of the backend and the way to show a target, for a name of the view */
  const prepare = (site: DeclarationSite, reveal: (place: DeclarationPlace) => boolean, from: DeclarationPlace | null) => {
    const repositoryPath = getDiffRepositoryPath()
    if (!repositoryPath) {
      return null
    }
    const sides = files.map(({ file, diff }) => ({
      path: file.path,
      oldPath: file.oldPath,
      oldId: diff?.oldId ?? file.oldId,
      newId: diff?.newId ?? file.newId,
      worktree: false,
    }))
    const show = (target: DeclarationTarget) => {
      if (target.outside !== null || !reveal(placeOf(target))) {
        openFile(target, from)
      }
    }
    const params: FindDeclarationParams = { repositoryPath, site, files: sides, revisions }
    return { params, show }
  }

  /** Asks the backend about a name of the view and answers with the way to show a target, or says why it cannot */
  const ask = <T>(
    kind: string,
    call: (params: FindDeclarationParams) => Promise<CommandResult<T>>,
    failure: string,
    answer: (data: T, show: (target: DeclarationTarget) => void, at: { x: number; y: number }) => void,
  ): SymbolAction => {
    return (site, at, reveal, from) => {
      const prepared = prepare(site, reveal, from)
      if (prepared === null) {
        return
      }
      const ticket = gate.start(`${kind}\n${site.path}\n${site.side}\n${site.blob.id}\n${site.line}:${site.column}`)
      if (ticket === null) {
        return
      }
      void invokeCommand(() => call(prepared.params)).then(result => {
        if (!gate.finish(ticket)) {
          return
        }
        if (result.status === "error") {
          notifyError(failure, result.error)
        } else {
          answer(result.data, prepared.show, at)
        }
      })
    }
  }
  const goToDeclaration = ask("declaration", commands.findDeclaration, "Cannot go to the declaration", ({ name, targets, indexError }, show, at) => {
    if (targets.length === 1) {
      show(targets[0]!)
    } else if (targets.length > 1) {
      setChoice({ kind: "declarations", name, at, targets, show, returnFocus: focused() })
    } else {
      showToast({ type: "info", title: name === "" ? "No name here" : `No declaration of ${name} found`, description: indexError ?? undefined })
    }
  })

  const findUsages = ask("usages", commands.findUsages, "Cannot find the usages", ({ name, usages, moreOutside, indexError }, show, at) => {
    // a declaration alone is no usage, as IntelliJ says "No usages found"
    if (usages.some(usage => usage.target.kind !== "declaration")) {
      setChoice({ kind: "usages", name, at, usages, moreOutside, indexError, show, returnFocus: focused() })
    } else {
      showToast({ type: "info", title: name === "" ? "No name here" : `No usages of ${name} found`, description: indexError ?? undefined })
    }
  })

  return { goToDeclaration, findUsages, choice, closeChoice: () => setChoice(null) }
}
