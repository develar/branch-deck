import { useEffect, useRef, useState } from "react"
import type { GitHubUser } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { createLatestOnly } from "@/lib/create-pull-request"

/** A search waits for the typing to pause */
const SEARCH_DEBOUNCE_MS = 250
/** As `MIN_SEARCH_LENGTH` of the backend: a shorter query is not sent */
export const MIN_SEARCH_LENGTH = 2

export interface ReviewerSearch {
  query: string
  setQuery: (query: string) => void
  results: readonly GitHubUser[]
  isSearching: boolean
  error: string | null
  open: boolean
  setOpen: (open: boolean) => void
  /** Searches `name` and shows what GitHub found */
  lookUp: (name: string) => void
}

/**
 * The search of the users of the repository a review can be asked from (`search_github_users`), for the Create Pull
 * Request dialog and Request Review…: debounced, and an older answer never replaces a newer one
 */
export function useReviewerSearch(repositoryPath: string): ReviewerSearch {
  const [query, setQueryState] = useState("")
  const [results, setResults] = useState<readonly GitHubUser[]>([])
  const [isSearching, setIsSearching] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [open, setOpen] = useState(false)
  const [latest] = useState(createLatestOnly)
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)

  useEffect(() => () => clearTimeout(timer.current), [])

  const setQuery = (next: string) => {
    setQueryState(next)
    clearTimeout(timer.current)
    const id = latest.next()
    if (next.trim().length < MIN_SEARCH_LENGTH) {
      setResults([])
      setIsSearching(false)
      setError(null)
      return
    }
    setIsSearching(true)
    timer.current = setTimeout(() => {
      commands.searchGithubUsers({ repositoryPath, query: next }).then(
        result => {
          if (!latest.isLatest(id)) {
            return
          }
          setIsSearching(false)
          if (result.status === "ok") {
            setResults(result.data)
            setError(null)
          } else {
            setError(result.error.message)
          }
        },
        (failure: unknown) => {
          if (latest.isLatest(id)) {
            setIsSearching(false)
            setError(failure instanceof Error ? failure.message : String(failure))
          }
        },
      )
    }, SEARCH_DEBOUNCE_MS)
  }

  return {
    query,
    setQuery,
    results,
    isSearching,
    error,
    open,
    setOpen,
    lookUp: name => {
      setQuery(name)
      setOpen(true)
    },
  }
}
