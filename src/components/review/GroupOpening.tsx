import { Badge } from "@/components/ui/badge"
import { RegistrationLine, TextsLine } from "@/components/review/ReviewMap"
import { StepFactsLine, UnprovenFact } from "@/components/review/StepFactsLine"
import type { GroupOpeningView } from "@/presenters/review-story"

/**
 * The opening of a group of a large step above its first card (spec/branch/review-tour.spec.md, "In the Review Window"):
 * its name (a module in the code font, its root in the hover), "New module" or "Deleted module", the small modules it
 * holds; then its facts a line each as a step's opening has them (the tests of the change that reach its code, its code
 * none reaches in the warning tone, its new API, what it opens wider, its new uses between modules), what its module
 * registers in the platform and the texts of the UI it adds. A card of prose, as the opening of a step
 */
export function GroupOpening({ opening }: { opening: GroupOpeningView }) {
  const { facts } = opening
  return (
    <div className="dv-prose dv-group-opening flex min-w-0 flex-col gap-1.5" data-testid="review-group-opening" data-group={opening.key}>
      <div className="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-0.5">
        <h3
          className={opening.code ? "min-w-0 truncate font-mono text-sm font-semibold" : "min-w-0 truncate text-sm font-semibold"}
          title={opening.root ?? undefined}
          data-testid="review-group-opening-name"
        >
          {opening.name}
        </h3>
        {opening.state !== null && (
          <Badge variant={opening.state === "New module" ? "success" : "secondary"} className="shrink-0" data-testid="review-group-opening-state">
            {opening.state}
          </Badge>
        )}
        {opening.also !== null && <span className="text-xs text-muted-foreground">{opening.also}</span>}
      </div>
      {facts.unproven !== null && <UnprovenFact unproven={facts.unproven} testId="review-group-opening-unproven" />}
      <StepFactsLine facts={facts} testId="review-group-opening-facts" lines />
      {opening.registrations.length > 0 && (
        <ul aria-label={`Registrations of ${opening.name}`} className="flex min-w-0 flex-col text-xs" data-testid="review-group-opening-registrations">
          {opening.registrations.map(registration => (
            <li key={registration.key} className="flex min-w-0">
              <RegistrationLine registration={registration} />
            </li>
          ))}
          {opening.moreRegistrations > 0 && (
            <li className="text-muted-foreground">{opening.moreRegistrations === 1 ? "and 1 more registration" : `and ${opening.moreRegistrations} more registrations`}</li>
          )}
        </ul>
      )}
      {opening.texts !== null && <TextsLine texts={opening.texts} fact />}
    </div>
  )
}
