import { useEffect, useState } from "react"
import { getErrorDetails } from "@/lib/errors"
import type { ReviewPatronus } from "@/lib/pull-request-review"
import { readPatronusTargetBranch, usePatronusAvailable } from "@/stores/patronus"
import { useSelectedProjectPath } from "@/stores/repository"

/**
 * Where Patronus merges in the selected repository, for Approve and Safe Push of a reviewed pull request in the menu of
 * a Your Turn row: the branch it pushes into is read while `active` (the card lists requests), once per repository
 */
export function useReviewPatronus(active: boolean): ReviewPatronus {
  const available = usePatronusAvailable()
  const repositoryPath = useSelectedProjectPath()
  // undefined while it is read; a failed read says why
  const [target, setTarget] = useState<{ repositoryPath: string | undefined; branch: string | null | undefined; error?: string } | undefined>(undefined)
  useEffect(() => {
    if (!active || !available) {
      return
    }
    let current = true
    readPatronusTargetBranch().then(
      branch => {
        if (current) {
          setTarget({ repositoryPath, branch })
        }
      },
      (error: unknown) => {
        if (current) {
          setTarget({ repositoryPath, branch: undefined, error: getErrorDetails(error) })
        }
      },
    )
    return () => {
      current = false
    }
  }, [active, available, repositoryPath])
  const own = target !== undefined && target.repositoryPath === repositoryPath ? target : undefined
  return { available, target: own?.branch, ...(own?.error !== undefined && { targetError: own.error }) }
}
