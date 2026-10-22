import { useRef } from "react"
import type { RefObject } from "react"
import { cn } from "cn"
import { SearchIcon, XIcon } from "lucide-react"
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from "@/components/ui/input-group"
import { Kbd } from "@/components/ui/kbd"
import { useSlashFocus } from "@/hooks/useSlashFocus"

export interface SearchFieldProps {
  value: string
  onValueChange: (value: string) => void
  /** The placeholder and the accessible name: "Filter branches" */
  label: string
  /** Escape in the empty field: the focus moves to what the field filters, whose keys then work */
  onLeave: () => void
  /** ↓ in the field: the focus moves to the first result */
  onArrowDown: () => void
  /** The width: `w-60` in a bar, `w-full` in a sidebar */
  className?: string
  inputRef?: RefObject<HTMLInputElement | null>
  /** `/` focuses it from anywhere in the window (on by default); off for a second search of a window, which has one `/` */
  slashFocus?: boolean
}

/**
 * The search of a list, the same wherever a list has one (the branch table, the files of a review): the search icon,
 * `/` focuses it from anywhere in the window (the `Kbd` at its end while empty), X clears a typed text, Escape clears
 * it, then leaves the field, ↓ moves to the first result.
 */
export function SearchField({ value, onValueChange, label, onLeave, onArrowDown, className, inputRef, slashFocus = true }: SearchFieldProps) {
  const ownRef = useRef<HTMLInputElement>(null)
  const ref = inputRef ?? ownRef
  useSlashFocus(ref, slashFocus)
  return (
    <InputGroup className={cn("h-7", className)}>
      <InputGroupAddon>
        <SearchIcon />
      </InputGroupAddon>
      <InputGroupInput
        ref={ref}
        type="text"
        value={value}
        placeholder={label}
        aria-label={label}
        autoComplete="off"
        spellCheck={false}
        className="h-7"
        onChange={event => onValueChange(event.target.value)}
        onKeyDown={event => {
          if (event.key === "Escape") {
            event.preventDefault()
            if (value === "") {
              onLeave()
            } else {
              onValueChange("")
            }
          } else if (event.key === "ArrowDown") {
            event.preventDefault()
            onArrowDown()
          }
        }}
      />
      <InputGroupAddon align="inline-end">
        {value === "" ? (
          slashFocus && <Kbd>/</Kbd>
        ) : (
          <InputGroupButton size="icon-xs" aria-label="Clear the filter text" onClick={() => onValueChange("")}>
            <XIcon />
          </InputGroupButton>
        )}
      </InputGroupAddon>
    </InputGroup>
  )
}
