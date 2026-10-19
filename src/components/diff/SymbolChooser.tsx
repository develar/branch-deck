import type { KeyboardEvent, ReactNode } from "react"
import { Popover, PopoverContent } from "@/components/ui/popover"
import type { SymbolChoice } from "@/hooks/useSymbolNavigation"
import type { DeclarationTarget, UsageTarget } from "@/lib/bindings"

/** The characters of a line shown before a name at most: a long line keeps its name in view */
const LEAD = 40

/**
 * What a name leads to, at the name (spec/commit/symbol-navigation.spec.md): its declarations, as IntelliJ's "Choose
 * Declaration", the most likely first, each with the declaration it is a member of and its file and line; or its
 * usages, as IntelliJ's "Show Usages", those of the review, then those outside it, each with how it uses the name, its
 * line and its file. ↑ and ↓ move, a click or Enter shows one
 */
export function SymbolChooser({ choice, onClose }: { choice: SymbolChoice | null; onClose: () => void }) {
  const at = choice?.at ?? { x: 0, y: 0 }
  const pick = (target: DeclarationTarget) => {
    onClose()
    choice?.show(target)
  }
  return (
    <Popover
      open={choice !== null}
      onOpenChange={(open, details) => {
        if (!open) {
          // Escape gives the focus back to the name; a choice moves it to the place it shows, which the view takes
          if (details.reason === "escape-key") {
            choice?.returnFocus?.focus({ preventScroll: true })
          }
          onClose()
        }
      }}
    >
      <PopoverContent
        anchor={{ getBoundingClientRect: () => DOMRect.fromRect({ x: at.x, y: at.y, width: 0, height: 0 }) }}
        side="bottom"
        align="start"
        className="w-auto max-w-2xl p-1"
        finalFocus={false}
        aria-label={choice === null ? undefined : choice.kind === "usages" ? `Usages of ${choice.name}` : `Choose Declaration of ${choice.name}`}
        data-testid="symbol-chooser"
      >
        {choice?.kind === "declarations" && (
          <div role="listbox" aria-label={`Declarations of ${choice.name}`} className="flex flex-col" onKeyDown={moveFocus}>
            {choice.targets.map((target, index) => (
              <Option key={`${target.path}:${target.line}:${target.column}`} first={index === 0} onPick={() => pick(target)}>
                <span className="font-mono">
                  {target.container === null ? "" : <span className="text-muted-foreground">{target.container}.</span>}
                  {choice.name}
                </span>
                <span className="truncate text-xs text-muted-foreground">
                  {target.path}:{target.line + 1}
                  {target.outside !== null && " · outside the review"}
                </span>
              </Option>
            ))}
          </div>
        )}
        {choice?.kind === "usages" && <UsageList name={choice.name} usages={choice.usages} moreOutside={choice.moreOutside} indexError={choice.indexError} onPick={pick} />}
      </PopoverContent>
    </Popover>
  )
}

function UsageList({
  name,
  usages,
  moreOutside,
  indexError,
  onPick,
}: {
  name: string
  usages: readonly UsageTarget[]
  moreOutside: boolean
  indexError: string | null
  onPick: (target: DeclarationTarget) => void
}) {
  const count = usages.filter(usage => usage.target.kind !== "declaration").length
  const groups = [
    { label: "In the review", usages: usages.filter(usage => usage.target.outside === null) },
    { label: "Outside the review", usages: usages.filter(usage => usage.target.outside !== null) },
  ].filter(group => group.usages.length > 0)
  return (
    <div className="flex flex-col gap-1">
      <div className="px-2 pt-1 text-xs text-muted-foreground">
        {count}
        {moreOutside && "+"} {count === 1 && !moreOutside ? "usage" : "usages"} of <span className="font-mono text-foreground">{name}</span>
      </div>
      <div role="listbox" aria-label={`Usages of ${name}`} className="flex max-h-96 flex-col gap-1 overflow-y-auto" onKeyDown={moveFocus}>
        {groups.map((group, groupIndex) => (
          <div key={group.label} role="group" aria-label={group.label} className="flex flex-col">
            {groups.length > 1 && <div className="px-2 py-0.5 text-xs font-medium text-muted-foreground">{group.label}</div>}
            {group.usages.map((usage, index) => (
              <Option key={`${usage.target.path}:${usage.target.line}:${usage.target.column}`} first={groupIndex === 0 && index === 0} onPick={() => onPick(usage.target)}>
                <span className="w-20 shrink-0 text-xs text-muted-foreground">{usage.target.kind}</span>
                <UsageLine usage={usage} />
                <span className="ml-auto shrink-0 pl-2 text-xs text-muted-foreground">
                  {usage.target.path}:{usage.target.line + 1}
                </span>
              </Option>
            ))}
          </div>
        ))}
      </div>
      {moreOutside && <div className="px-2 pb-1 text-xs text-muted-foreground">The index has more usages outside the review</div>}
      {indexError !== null && <div className="px-2 pb-1 text-xs text-warning">{indexError}: the usages outside the review are missing</div>}
    </div>
  )
}

/** The line of a usage with its name in bold, the start of a long line cut */
function UsageLine({ usage }: { usage: UsageTarget }) {
  const { text, textColumn, target } = usage
  const end = textColumn + (target.endColumn - target.column)
  const start = textColumn > LEAD ? textColumn - LEAD : 0
  return (
    <span className="min-w-0 truncate font-mono">
      {start > 0 && "…"}
      {text.slice(start, textColumn)}
      <span className="font-semibold">{text.slice(textColumn, end)}</span>
      {text.slice(end)}
    </span>
  )
}

function Option({ first, onPick, children }: { first: boolean; onPick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={false}
      autoFocus={first}
      className="flex items-baseline gap-3 rounded-sm px-2 py-1 text-left text-sm hover:bg-muted focus:bg-muted focus:outline-none"
      onClick={onPick}
    >
      {children}
    </button>
  )
}

/** ↑ and ↓ move the focus between the options of a list, across its groups */
function moveFocus(event: KeyboardEvent<HTMLElement>) {
  const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0
  if (step === 0) {
    return
  }
  const options = [...event.currentTarget.querySelectorAll<HTMLElement>("[role=option]")]
  const next = options[options.indexOf(document.activeElement as HTMLElement) + step]
  if (next !== undefined) {
    event.preventDefault()
    next.focus()
  }
}
