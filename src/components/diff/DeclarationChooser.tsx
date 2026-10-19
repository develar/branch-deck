import { Popover, PopoverContent } from "@/components/ui/popover"
import type { DeclarationChoice } from "@/hooks/useGoToDeclaration"

/**
 * The declarations a name may mean, at the name, as IntelliJ's "Choose Declaration": the most likely first, each with
 * the declaration it is a member of and its file and line; a click or Enter shows it
 */
export function DeclarationChooser({ choice, onClose }: { choice: DeclarationChoice | null; onClose: () => void }) {
  const at = choice?.at ?? { x: 0, y: 0 }
  return (
    <Popover open={choice !== null} onOpenChange={open => !open && onClose()}>
      <PopoverContent
        anchor={{ getBoundingClientRect: () => DOMRect.fromRect({ x: at.x, y: at.y, width: 0, height: 0 }) }}
        side="bottom"
        align="start"
        className="w-auto max-w-xl p-1"
        aria-label={choice === null ? undefined : `Choose Declaration of ${choice.name}`}
        data-testid="declaration-chooser"
      >
        {choice !== null && (
          <div role="listbox" aria-label={`Declarations of ${choice.name}`} className="flex flex-col">
            {choice.targets.map((target, index) => (
              <button
                key={`${target.path}:${target.line}:${target.column}`}
                type="button"
                role="option"
                aria-selected={false}
                autoFocus={index === 0}
                className="flex items-baseline gap-3 rounded-sm px-2 py-1 text-left text-sm hover:bg-muted focus:bg-muted focus:outline-none"
                onClick={() => {
                  onClose()
                  choice.show(target)
                }}
                onKeyDown={event => {
                  const next = event.key === "ArrowDown" ? event.currentTarget.nextElementSibling : event.key === "ArrowUp" ? event.currentTarget.previousElementSibling : null
                  if (next instanceof HTMLElement) {
                    event.preventDefault()
                    next.focus()
                  }
                }}
              >
                <span className="font-mono">
                  {target.container === null ? "" : <span className="text-muted-foreground">{target.container}.</span>}
                  {choice.name}
                </span>
                <span className="truncate text-xs text-muted-foreground">
                  {target.path}:{target.line + 1}
                  {target.outside !== null && " · outside the review"}
                </span>
              </button>
            ))}
          </div>
        )}
      </PopoverContent>
    </Popover>
  )
}
