import { useState } from "react"
import type { DeclarationPlace, DiffViewFile } from "@/components/diff/DiffView"
import { commands } from "@/lib/bindings"
import type { DeclarationSite, DeclarationTarget, DiffRevisions } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { showToast } from "@/lib/toast-message"
import { getDiffRepositoryPath } from "@/stores/repository"

/** The declarations a name may mean, for the user to choose one (`DeclarationChooser`) */
export interface DeclarationChoice {
  name: string
  at: { x: number; y: number }
  targets: readonly DeclarationTarget[]
  /** Shows a target: in the view, or a file outside the review in a view of its own */
  show: (target: DeclarationTarget) => void
}

/** The place of a target in the view */
export function placeOf(target: DeclarationTarget): DeclarationPlace {
  return { path: target.path, side: target.side, line: target.line, column: target.column, endColumn: target.endColumn }
}

/**
 * Go to Declaration in a review (spec/commit/symbol-navigation.spec.md): the backend finds the declarations of the
 * name in the files of the review, else in the index of the repository; one is shown at once (a file outside the review
 * in a view of its own), several are offered, none is said in a toast
 */
export function useGoToDeclaration(files: readonly DiffViewFile[], revisions: DiffRevisions, openFile: (target: DeclarationTarget, from: DeclarationPlace | null) => void) {
  const [choice, setChoice] = useState<DeclarationChoice | null>(null)
  const goToDeclaration = (site: DeclarationSite, at: { x: number; y: number }, reveal: (place: DeclarationPlace) => boolean, from: DeclarationPlace | null) => {
    const repositoryPath = getDiffRepositoryPath()
    if (!repositoryPath) {
      return
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
    void invokeCommand(() => commands.findDeclaration({ repositoryPath, site, files: sides, revisions })).then(result => {
      if (result.status === "error") {
        notifyError("Cannot go to the declaration", result.error)
        return
      }
      const { name, targets } = result.data
      if (targets.length === 1) {
        show(targets[0]!)
      } else if (targets.length > 1) {
        setChoice({ name, at, targets, show })
      } else {
        showToast({ type: "info", title: name === "" ? "No name here" : `No declaration of ${name} found` })
      }
    })
  }
  return { goToDeclaration, choice, closeChoice: () => setChoice(null) }
}
