import type { RefObject } from "react"
import { ChevronDownIcon, ChevronUpIcon, SearchIcon, XIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"

export interface DiffFindBarProps {
  query: string
  onQueryChange: (query: string) => void
  /** The hits of the query, and the index of the current one: -1 before the first step */
  count: number
  current: number
  /** The folded lines are searched too */
  folded: boolean
  onFoldedChange: (folded: boolean) => void
  /** To the next (1) or the previous (-1) hit */
  onStep: (direction: 1 | -1) => void
  onClose: () => void
  inputRef: RefObject<HTMLInputElement | null>
}

/**
 * The find bar above the diffs of a review (docs/design/diff-view.md, "Find in the diffs"): Enter and ⌘G go to the next
 * hit, with Shift to the previous one; Escape closes the bar
 */
export function DiffFindBar({ query, onQueryChange, count, current, folded, onFoldedChange, onStep, onClose, inputRef }: DiffFindBarProps) {
  return (
    <div role="search" data-testid="diff-find-bar" className="flex shrink-0 items-center gap-2 border-b border-border bg-card px-2 py-1">
      <InputGroup className="h-7 w-72">
        <InputGroupAddon>
          <SearchIcon />
        </InputGroupAddon>
        <InputGroupInput
          ref={inputRef}
          type="text"
          value={query}
          placeholder="Find in the diffs"
          aria-label="Find in the diffs"
          autoComplete="off"
          spellCheck={false}
          className="h-7"
          onChange={event => onQueryChange(event.target.value)}
          onKeyDown={event => {
            if (event.key === "Enter") {
              event.preventDefault()
              onStep(event.shiftKey ? -1 : 1)
            } else if (event.key === "Escape") {
              event.preventDefault()
              onClose()
            }
          }}
        />
      </InputGroup>
      <span data-testid="diff-find-count" className="min-w-16 text-xs text-muted-foreground tabular-nums">
        {query === "" ? "" : count === 0 ? "No results" : current < 0 ? `${count} ${count === 1 ? "result" : "results"}` : `${current + 1} of ${count}`}
      </span>
      <Button variant="ghost" size="icon-xs" aria-label="Previous match" disabled={count === 0} onClick={() => onStep(-1)}>
        <ChevronUpIcon />
      </Button>
      <Button variant="ghost" size="icon-xs" aria-label="Next match" disabled={count === 0} onClick={() => onStep(1)}>
        <ChevronDownIcon />
      </Button>
      <label className="flex items-center gap-1.5 text-xs text-muted-foreground">
        <Checkbox checked={folded} onCheckedChange={checked => onFoldedChange(checked)} />
        In folded lines
      </label>
      <Button variant="ghost" size="icon-xs" className="ml-auto" aria-label="Close the find" onClick={onClose}>
        <XIcon />
      </Button>
    </div>
  )
}
