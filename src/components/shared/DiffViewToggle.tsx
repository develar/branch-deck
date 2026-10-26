import { cn } from "cn"
import { AlignLeftIcon, ChevronDownIcon, Columns2Icon, PilcrowIcon, StrikethroughIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { DropdownMenu, DropdownMenuContent, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import type { WhitespacePolicy } from "@/lib/diff/file-diff-model"
import { setDiffViewMode, setDiffWhitespacePolicy, useDiffViewMode, useDiffWhitespacePolicy } from "@/stores/settings/diff-view-settings"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"

/** The layouts in IntelliJ's order: side by side (the default), unified, then Inline beside it as its compact variant */
const modes: Array<{ mode: DiffViewMode; label: string; icon: typeof AlignLeftIcon }> = [
  { mode: "split", label: "Split", icon: Columns2Icon },
  { mode: "unified", label: "Unified", icon: AlignLeftIcon },
  { mode: "inline", label: "Inline", icon: StrikethroughIcon },
]

/** IntelliJ's choices of what the diff ignores, in its order */
const WHITESPACE_POLICIES: Array<{ policy: WhitespacePolicy; label: string }> = [
  { policy: "default", label: "Do Not Ignore" },
  { policy: "trim", label: "Trim Whitespace" },
  { policy: "ignore", label: "Ignore Whitespace" },
  { policy: "ignoreBlankLines", label: "Ignore Whitespace and Blank Lines" },
]

/** The layout switch of every diff view (`diff-view-settings.ts`) and the Whitespace menu beside it, above the diffs */
export function DiffViewToggle() {
  return (
    <div className="flex items-center gap-2">
      <DiffLayoutButtons />
      <WhitespaceMenu />
    </div>
  )
}

/**
 * Split · Unified · Inline, one option set (the stock `ToggleGroup`), the pressed one the layout of every diff view; also
 * in Settings → Diffs & Merge. A press of the pressed one keeps it
 */
export function DiffLayoutButtons() {
  const current = useDiffViewMode()
  return (
    <ToggleGroup
      aria-label="Diff layout"
      variant="outline"
      size="sm"
      spacing={0}
      value={[current]}
      onValueChange={next => {
        const chosen = next.find(it => it !== current)
        if (chosen !== undefined) {
          setDiffViewMode(chosen as DiffViewMode)
        }
      }}
    >
      {modes.map(({ mode, label, icon: Icon }) => (
        <ToggleGroupItem key={mode} value={mode}>
          <Icon data-icon="inline-start" />
          {label}
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  )
}

/**
 * "Whitespace": what the diff engine ignores; the button is marked while it ignores anything. `named`: the button
 * says the choice instead, as a row of Settings → Diffs & Merge labelled "Whitespace"
 */
export function WhitespaceMenu({ named = false }: { named?: boolean }) {
  const policy = useDiffWhitespacePolicy()
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button size="xs" variant="outline" className={cn(!named && policy !== "default" && "border-primary/50 text-primary hover:text-primary")} />}>
        <PilcrowIcon data-icon="inline-start" />
        {named ? WHITESPACE_POLICIES.find(it => it.policy === policy)?.label : "Whitespace"}
        <ChevronDownIcon data-icon="inline-end" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup value={policy} onValueChange={value => setDiffWhitespacePolicy(value as WhitespacePolicy)}>
          {WHITESPACE_POLICIES.map(({ policy: value, label }) => (
            <DropdownMenuRadioItem key={value} value={value}>
              {label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
