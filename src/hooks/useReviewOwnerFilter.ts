import { useState } from "react"
import { awaitsRememberedChip, canonicalOwnerFilter, initialOwnerFilter } from "@/lib/review-owners"
import type { ReviewFilter, ReviewFilterChip } from "@/lib/review-owners"
import { setReviewOwnerChip, useRecentProjects } from "@/stores/settings/repositorySettings"

/** What a review window opens on: the chips it has now, whether the user's group comes before All, a chip the open names */
export interface OwnerFilterStart {
  readonly chips: readonly ReviewFilterChip[]
  /** The pull request window: the user's group that still needs an approval when no remembered chip marks files */
  readonly preferYours: boolean
  /** The chip the open names (Review Changes of the Safe Push popover at a group): it wins over the remembered one */
  readonly named: ReviewFilter | null
}

/** The chip of one scope: the remembered one as the scope began, and the chip it holds (null: not settled yet) */
interface Held {
  readonly scope: string
  readonly remembered: ReviewFilter | undefined
  readonly filter: ReviewFilter | null
}

/**
 * The owner chip of a review window (spec/branch/branch-review.spec.md, "Code owners"), per `scope` (an open of the
 * window on a request): until the user chooses one, the chip it opens on (`initialOwnerFilter`) of the remembered chip
 * as the scope began, so a choice in another review does not move it. While that is All, or the user's group while the
 * remembered chip may still come (`awaitsRememberedChip`), the chips that come later (the groups of the readiness of the
 * request, the owners) choose again; once it is another chip, it holds. A chip the user
 * chooses holds, All too, and is remembered for the repository; All forgets it. A chip held as an owner of the
 * `OWNERSHIP` files is the group of its name once the groups come (`canonicalOwnerFilter`): one chip, the group's.
 */
export function useReviewOwnerFilter(repositoryPath: string, scope: string, start: OwnerFilterStart): readonly [ReviewFilter, (filter: ReviewFilter) => void] {
  const remembered = useRecentProjects(projects => projects.find(it => it.path === repositoryPath)?.reviewOwnerChip)
  const [held, setHeld] = useState<Held>({ scope, remembered, filter: start.named })
  let current = held
  if (current.scope !== scope) {
    current = { scope, remembered, filter: null }
    setHeld(current)
  }
  let filter = current.filter
  if (filter === null) {
    filter = initialOwnerFilter(start.chips, current.remembered, start.preferYours)
    // the remembered chip may come with the owners, after the groups: the user's group holds only once it can't
    if (filter.kind !== "all" && !awaitsRememberedChip(start.chips, current.remembered)) {
      setHeld({ ...current, filter })
    }
  }
  filter = canonicalOwnerFilter(start.chips, filter)
  const choose = (next: ReviewFilter) => {
    setHeld({ scope, remembered, filter: next })
    setReviewOwnerChip(repositoryPath, next)
  }
  return [filter, choose]
}
