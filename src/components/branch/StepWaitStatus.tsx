import { cn } from "cn"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { formatKbds, FROM_MENU } from "@/lib/menu"
import type { NextStepTone, StepWait } from "@/presenters/next-step"

/** O of the focused row, which opens the pull request the branch waits in */
const OPEN_KEY = formatKbds(["o"])

const toneClass: Readonly<Record<NextStepTone, string>> = {
  default: "text-muted-foreground",
  quiet: "text-muted-foreground",
  destructive: "text-destructive",
}

/**
 * The state a subject waits in, as a status says it ("◷ Awaiting review", "Changes requested" in `destructive`,
 * "Draft"): its icon where a status has its dot, the words muted and truncated to the track, the whole state on hover
 * with what a click opens ("Open Pull Request #1078", and O on the focused row where `showKeys`). A status, not an
 * action: the click only opens the pull request it waits in, as the robot chip opens its robot
 * ([Next Step](../../../openspec/specs/branch/next-step/spec.md), "A wait is a state")
 */
export function StepWaitStatus({ wait, showKeys }: { wait: StepWait; showKeys: boolean }) {
  const Icon = wait.icon
  return (
    <div className="min-w-0">
      <Tooltip lazy>
        <TooltipTrigger
          delay={300}
          render={
            <button
              type="button"
              className={cn("flex h-6 max-w-full items-center gap-2 rounded-sm text-sm outline-none hover:underline focus-visible:bd-focus", toneClass[wait.tone])}
              data-testid="step-wait"
              data-step-wait
              data-tone={wait.tone}
              onClick={event => {
                // the row toggles on a click
                event.stopPropagation()
                wait.open(FROM_MENU)
              }}
            />
          }
        >
          {Icon !== undefined && <Icon aria-hidden className="size-3.5 shrink-0" />}
          <span className="truncate">{wait.label}</span>
        </TooltipTrigger>
        <TooltipContent
          className="max-w-sm break-words whitespace-pre-line"
          data-testid="step-wait-hover"
          description={[wait.description, showKeys ? `Shortcut: ${OPEN_KEY}` : ""].filter(words => words !== "").join(" · ") || false}
        >
          {wait.description}
          {showKeys && <Kbd>{OPEN_KEY}</Kbd>}
        </TooltipContent>
      </Tooltip>
    </div>
  )
}
