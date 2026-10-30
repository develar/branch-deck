import { useEffect } from "react"
import type { ReactNode, RefObject } from "react"
import { ArrowDownIcon, ArrowUpIcon, RefreshCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Spinner } from "@/components/ui/spinner"
import { HourChart } from "@/components/landing/charts/HourChart"
import { KindChart } from "@/components/landing/charts/KindChart"
import { MergeTimeChart } from "@/components/landing/charts/MergeTimeChart"
import { RankedList } from "@/components/landing/charts/RankedList"
import { Sparkline } from "@/components/landing/charts/Sparkline"
import { TimeChart } from "@/components/landing/charts/TimeChart"
import { WeekChart } from "@/components/landing/charts/WeekChart"
import { Callout } from "@/components/shared/Callout"
import { ItemSelect } from "@/components/shared/ItemSelect"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { RelativeTime } from "@/components/shared/RelativeTime"
import { useRobotStats } from "@/hooks/useRobotStats"
import type { RobotStats, StatsPeriod } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import { formatTotalDuration } from "@/lib/duration"
import { formatSafePushLoginCommand } from "@/lib/patronus"
import { isWindows } from "@/lib/platform"
import { formatShortDuration } from "@/lib/robot-eta"
import { STATS_PERIODS, compareToEveryone, formatHour, formatRatio, formatShare, formatShareCount, formatStatsPeriod, hasMergeTimes } from "@/lib/robot-stats"
import { useSelectedProjectPath } from "@/stores/repository"
import type { RobotStatsProgress } from "@/stores/robot-stats"

/**
 * The Landing view of the main window (`spec/repository/main-views.spec.md`): how the Safe Pushes of the repository
 * went in the chosen period, from `get_robot_stats`. Read only while it shows; a new answer replaces the numbers whole,
 * nothing moves while it loads (the numbers of another period stay dimmed until those of the chosen one come), and a
 * read nobody asked for fails as a state of the view, not a toast.
 */
export function LandingView({ visible, headingRef }: { visible: boolean; headingRef: RefObject<HTMLHeadingElement | null> }) {
  const { result, partial, progress, loading, error, period, setPeriod, refresh } = useRobotStats(visible)
  // the first show loads this chunk after the switch: the focus the hidden branches dropped comes to the heading here
  useEffect(() => {
    if (document.activeElement === null || document.activeElement === document.body) {
      headingRef.current?.focus({ preventScroll: true })
    }
  }, [headingRef])
  const kept = result?.stats ?? null
  const ofPeriod = (stats: RobotStats | null) => stats !== null && stats.window.period === period
  // the last answer of the chosen period, else the lists of its read, else what there is, dimmed
  const stats = ofPeriod(kept) ? kept : ofPeriod(partial) ? partial : (kept ?? partial)
  const current = ofPeriod(stats)
  const dimmed = !current || (loading && stats === kept)
  const availability = result?.availability.status
  const choosable = stats !== null || (availability !== "notPatronus" && availability !== "notLoggedIn")
  return (
    <div className="@container flex flex-col gap-2" data-testid="landing-view" aria-busy={loading}>
      <div className="flex min-h-9 items-center gap-3 px-1">
        <h1 ref={headingRef} tabIndex={-1} className="text-sm font-semibold text-foreground outline-none">
          Safe Push Statistics
        </h1>
        {choosable && <PeriodSelect period={period} onChange={setPeriod} />}
        {stats !== null && (
          <span className="min-w-0 truncate text-xs text-muted-foreground" data-testid="landing-window">
            {`Into ${stats.window.target} · ${stats.window.time_zone}`}
          </span>
        )}
        <div className="ml-auto flex items-center gap-2">
          {loading && progress !== null && <span className="text-xs text-muted-foreground">{formatProgress(progress)}</span>}
          <Button variant="ghost" size="sm" disabled={loading} onClick={refresh} data-testid="landing-refresh">
            {loading ? <Spinner data-icon="inline-start" /> : <RefreshCwIcon data-icon="inline-start" />}
            Refresh
          </Button>
        </div>
      </div>
      {result?.stale === true && current && result.availability.status === "error" && (
        <Callout tone="warning" title="Patronus did not answer: the numbers are of what was read before" data-testid="landing-stale">
          {result.availability.message}
          {result.fetched_at !== null && (
            <>
              {" · read "}
              <RelativeTime time={result.fetched_at * 1000} />
            </>
          )}
        </Callout>
      )}
      <LandingState stats={stats} result={result} loading={loading} error={error} period={period} />
      {stats !== null && stats.coverage.robots > 0 && (
        // the previous numbers stay while the next read runs, dimmed, nothing moves (no skeleton)
        <div className={dimmed ? "opacity-60 transition-opacity" : "transition-opacity"} data-testid="landing-numbers" data-period={stats.window.period}>
          <StatsCards stats={stats} runsLoading={stats === partial} />
        </div>
      )}
    </div>
  )
}

/** The period of the numbers: every chart and figure below counts it */
function PeriodSelect({ period, onChange }: { period: StatsPeriod; onChange: (period: StatsPeriod) => void }) {
  const items = STATS_PERIODS.map(value => ({ value, label: `Last ${formatStatsPeriod(value)}` }))
  return <ItemSelect items={items} value={period} onChange={onChange} testId="landing-period" ariaLabel="Period" align="start" />
}

function formatProgress(progress: RobotStatsProgress): string {
  if (progress.runs_total > 0) {
    return `Reading the checks of ${progress.runs_read} of ${progress.runs_total} robots…`
  }
  return progress.lists_total === 0 ? "Reading…" : `Listing ${progress.lists_read} of ${progress.lists_total} weeks…`
}

/** Why there are no numbers, or nothing */
function LandingState({ stats, result, loading, error, period }: Pick<ReturnType<typeof useRobotStats>, "result" | "loading" | "error" | "period"> & { stats: RobotStats | null }) {
  const repositoryPath = useSelectedProjectPath()
  if (stats !== null) {
    return stats.coverage.robots === 0 ? <StateCard>{`No Safe Push into master in the last ${formatStatsPeriod(stats.window.period)}`}</StateCard> : null
  }
  if (error !== null) {
    return (
      <Callout tone="destructive" title="Cannot read the Safe Push statistics" data-testid="landing-error">
        {error}
      </Callout>
    )
  }
  if (result === null) {
    return loading ? <LoadingLine className="px-1">{`Reading the robots of the last ${formatStatsPeriod(period)}…`}</LoadingLine> : null
  }
  switch (result.availability.status) {
    case "notPatronus":
      return <StateCard>This repository is not on Space: Patronus has no robots of it</StateCard>
    case "notLoggedIn":
      return (
        <StateCard
          action={
            repositoryPath === undefined ? undefined : (
              <Button variant="outline" size="sm" onClick={() => void copyText(formatSafePushLoginCommand(repositoryPath, isWindows))}>
                Copy Command
              </Button>
            )
          }
        >
          Run ./safePush.cmd once to log in to Patronus
        </StateCard>
      )
    case "error":
      return (
        <Callout tone="destructive" title="Cannot read the robots" data-testid="landing-error">
          {result.availability.message}
        </Callout>
      )
    case "available":
      return <StateCard>{`No Safe Push into master in the last ${formatStatsPeriod(period)}`}</StateCard>
  }
}

function StateCard({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return (
    <Card className="flex-row items-center justify-between gap-3 px-4 py-3 text-sm text-muted-foreground sm:px-6" data-testid="landing-state">
      <span>{children}</span>
      {action}
    </Card>
  )
}

/**
 * The islands, each one question: Into master (what landed, what it cost, where the time went) on the full width, then
 * the two charts by week side by side, then why robots failed beside when they run fastest. Each island is as tall as
 * its content, so the page holds the default period without a scroll at a usual window
 */
function StatsCards({ stats, runsLoading }: { stats: RobotStats; runsLoading: boolean }) {
  const mergeTimes = hasMergeTimes(stats.mine.weeks)
  return (
    <div className="grid gap-2 @4xl:grid-cols-2">
      <DeliveryCard stats={stats} />
      <StatsCard title="Robots per week" testId="landing-weeks" className={mergeTimes ? undefined : "@4xl:col-span-2"}>
        <WeekChart weeks={stats.mine.weeks} />
      </StatsCard>
      {mergeTimes && (
        <StatsCard title="Time to merge by week" note="you against everyone else's merges of the same week" testId="landing-merge-times">
          <MergeTimeChart weeks={stats.mine.weeks} />
        </StatsCard>
      )}
      <FailuresCard stats={stats} busy={runsLoading} />
      <HoursCard stats={stats} />
    </div>
  )
}

/** An island with its 40px header row */
function StatsCard({ title, note, testId, busy, className, children }: { title: string; note?: string; testId: string; busy?: boolean; className?: string; children: ReactNode }) {
  return (
    <Card className={className === undefined ? "gap-0 py-0" : `gap-0 py-0 ${className}`} data-testid={testId} aria-busy={busy}>
      <div className="flex items-center gap-2 px-4 py-2.5 sm:px-6">
        <h2 className="shrink-0 text-sm font-semibold whitespace-nowrap text-foreground">{title}</h2>
        {note !== undefined && <span className="truncate text-xs text-muted-foreground">{note}</span>}
        {busy === true && <Spinner className="ml-auto size-3.5 text-muted-foreground" />}
      </div>
      <div className="flex flex-col gap-4 px-4 pb-4 sm:px-6">{children}</div>
    </Card>
  )
}

/**
 * A stat tile: the name, the value (proportional figures, as a standalone number), how it compares to everyone else
 * (an arrow and words in the status colour of the direction, never the colour alone), the detail and the trend over the
 * weeks. The tile spans the five rows of the tiles grid, so the values, the comparisons and the trends of the row of
 * tiles stand on common lines whatever wraps
 */
function Figure({
  label,
  value,
  comparison,
  detail,
  trend,
  testId,
}: {
  label: string
  value: string
  comparison?: ReturnType<typeof compareToEveryone>
  detail?: ReactNode
  trend?: ReactNode
  testId: string
}) {
  const Arrow = comparison?.text.includes("more") === true ? ArrowUpIcon : ArrowDownIcon
  return (
    <div className="row-span-5 grid min-w-0 grid-rows-subgrid" data-testid={testId}>
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-2xl font-semibold text-foreground">{value}</span>
      {comparison === undefined || comparison === null ? (
        <span />
      ) : (
        <span
          className={
            comparison.better === null
              ? "text-xs text-muted-foreground"
              : comparison.better
                ? "flex items-center gap-1 text-xs text-success"
                : "flex items-center gap-1 text-xs text-destructive"
          }
          data-testid={`${testId}-comparison`}
        >
          {comparison.better !== null && <Arrow aria-hidden className="size-3 shrink-0" />}
          {comparison.text}
        </span>
      )}
      {detail === undefined ? <span /> : <span className="text-xs text-muted-foreground">{detail}</span>}
      {trend ?? <span />}
    </div>
  )
}

function DeliveryCard({ stats }: { stats: RobotStats }) {
  const { mine, everyone, first_try: firstTry, value, time } = stats
  const outcomes = [mine.failed > 0 && `${mine.failed} failed`, mine.canceled > 0 && `${mine.canceled} canceled`].filter(Boolean)
  const apart = [mine.dry_runs > 0 && `+${mine.dry_runs} ${mine.dry_runs === 1 ? "dry run" : "dry runs"}`, mine.emergency > 0 && `+${mine.emergency} emergency`].filter(Boolean)
  const lost = time.failed + time.canceled + time.between
  const total = lost + time.merged
  const lines = [
    value.commits_per_merge !== null && `${formatRatio(value.commits_per_merge)} commits per merge (the last 14 days)`,
    value.whole_stack > 0 && `${value.whole_stack} ${value.whole_stack === 1 ? "merge" : "merges"} of a whole stack`,
  ].filter(Boolean)
  return (
    <StatsCard
      title="Into master"
      note={everyone === null ? undefined : `against everyone else, the median of ${everyone.people} people`}
      testId="landing-delivery"
      className="@4xl:col-span-2"
    >
      <div className="grid grid-cols-2 gap-x-4 gap-y-1 @2xl:grid-cols-3 @4xl:grid-cols-5">
        <Figure
          label="Merged"
          value={String(mine.merged)}
          detail={
            outcomes.length + apart.length > 0 ? (
              <span className="flex flex-col">
                {outcomes.length > 0 && <span>{outcomes.join(" · ")}</span>}
                {apart.length > 0 && <span>{apart.join(" · ")}</span>}
              </span>
            ) : undefined
          }
          trend={<Sparkline id="merged-trend" values={fullWeeks(mine.weeks).map(week => week.merged)} label="Merges per week" />}
          testId="landing-merged"
        />
        <Figure
          label="Robots per merge"
          value={formatRatio(mine.robots_per_merge)}
          comparison={compareToEveryone(mine.robots_per_merge, everyone?.robots_per_merge ?? null)}
          detail={everyone === null ? undefined : `everyone else ${formatRatio(everyone.robots_per_merge)}`}
          trend={<Sparkline id="robots-per-merge-trend" values={mine.weeks.map(week => (week.merged === 0 ? null : week.robots / week.merged))} label="Robots per merge by week" />}
          testId="landing-robots-per-merge"
        />
        <Figure
          label="Time to merge"
          value={mine.merge_time === null ? "–" : formatShortDuration(mine.merge_time.median)}
          comparison={compareToEveryone(mine.merge_time?.median ?? null, everyone?.merge_time ?? null)}
          detail={[mine.merge_time !== null && `p90 ${formatShortDuration(mine.merge_time.p90)}`, everyone !== null && `everyone else ${formatShortDuration(everyone.merge_time)}`]
            .filter(Boolean)
            .join(" · ")}
          trend={<Sparkline id="merge-time-trend" values={mine.weeks.map(week => week.merge_time)} label="Median time to merge by week" />}
          testId="landing-merge-time"
        />
        <Figure
          label="First try"
          value={formatShare(firstTry)}
          detail={firstTry === null ? undefined : `${formatShareCount(firstTry)} merges, estimated`}
          testId="landing-first-try"
        />
        {total > 0 && (
          <Figure
            label="Lost to failures"
            value={formatTotalDuration(lost)}
            detail={`${Math.round((lost / total) * 100)}% of the robot time`}
            trend={<Sparkline id="lost-trend" values={fullWeeks(mine.weeks).map(week => week.lost)} label="Time lost by week" />}
            testId="landing-time-lost"
          />
        )}
      </div>
      {total > 0 && (
        <section aria-label="Where the time went" className="flex flex-col gap-2" data-testid="landing-time">
          <h3 className="text-xs font-medium text-muted-foreground">Where the time went: failed and canceled robots, and the waits before their restarts</h3>
          <TimeChart time={time} />
        </section>
      )}
      {lines.length > 0 && (
        <p className="text-xs text-muted-foreground" data-testid="landing-value">
          {lines.join(" · ")}
        </p>
      )}
    </StatsCard>
  )
}

/** The weeks that ended: a count of the week of now is of its days so far, and would read as a drop */
function fullWeeks(weeks: RobotStats["mine"]["weeks"]): RobotStats["mine"]["weeks"] {
  return weeks.filter(week => !week.current)
}

/** Why the robots failed, in three ranked lists of one language: what failed, the checks that failed the most, the flaky ones */
function FailuresCard({ stats, busy }: { stats: RobotStats; busy: boolean }) {
  const { failures, coverage } = stats
  return (
    <StatsCard title="Why robots failed" note="the last 14 days" testId="landing-failures" busy={busy}>
      <div className="grid gap-4 @2xl:grid-cols-2 @4xl:grid-cols-3">
        {failures.failed === 0 ? <p className="text-sm text-muted-foreground">No robot failed</p> : <KindChart kinds={failures.kinds} failed={failures.failed} />}
        <RankedList
          title="Failed the most"
          testId="landing-failed-checks"
          tone="failed"
          scale="first"
          items={failures.checks.map(check => ({ name: check.name, count: check.count, value: String(check.count), url: check.robot_url, robot: "failed" }))}
        />
        <RankedList
          title="Passed on a retry"
          testId="landing-flaky-checks"
          tone="merged"
          scale="first"
          items={failures.flaky.map(check => ({
            name: check.name,
            count: check.count,
            value: check.share === null ? `${check.count} of ${check.runs}` : `${Math.round(check.share * 100)}% · ${check.count} of ${check.runs}`,
            url: check.robot_url,
            robot: "passed on a retry",
          }))}
        />
      </div>
      {coverage.runs_missing > 0 && (
        <p className="text-xs text-muted-foreground" data-testid="landing-runs-missing">
          {`The checks of ${coverage.runs_missing} robots are not read yet: the next read asks for them`}
        </p>
      )}
    </StatsCard>
  )
}

function HoursCard({ stats }: { stats: RobotStats }) {
  const known = stats.start_hours.some(hour => hour.everyone !== null || hour.mine !== null)
  return (
    <StatsCard title="When robots run fastest" note={`workdays, by the hour a merged robot started (${stats.window.time_zone})`} testId="landing-hours">
      {stats.fastest_hours !== null && (
        <p className="text-sm text-foreground" data-testid="landing-fastest-hours">
          {`Robots that start ${formatHour(stats.fastest_hours.start)}–${formatHour(stats.fastest_hours.end)} merge fastest: ${formatShortDuration(stats.fastest_hours.median)}, against ${formatShortDuration(stats.fastest_hours.slowest_median)} at ${formatHour(stats.fastest_hours.slowest)}`}
        </p>
      )}
      {known ? <HourChart hours={stats.start_hours} fastest={stats.fastest_hours} /> : <p className="text-sm text-muted-foreground">Too few merged robots to tell an hour</p>}
    </StatsCard>
  )
}
