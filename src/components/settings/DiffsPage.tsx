import { useId } from "react"
import type { ReactNode } from "react"
import { SettingRow, SettingsSection } from "@/components/settings/SettingRow"
import { DiffLayoutButtons, WhitespaceMenu } from "@/components/shared/DiffViewToggle"
import { setDiffLigatures, setDiffSoftWraps, useDiffLigatures, useDiffSoftWraps } from "@/stores/settings/diff-view-settings"

/** Settings → Diffs & Merge: how every diff of the app shows its lines and what it ignores */
export function DiffsPage() {
  return (
    <SettingsSection title="Diffs" description="Every diff of the app: the commit preview, the reviews, the conflicts">
      <DiffSettings />
    </SettingsSection>
  )
}

/** The layout and the Whitespace menu, the controls above every diff too, then soft wraps and ligatures */
function DiffSettings() {
  const softWraps = useDiffSoftWraps()
  const ligatures = useDiffLigatures()
  return (
    <div className="flex flex-col gap-3">
      <ControlRow testId="setting-diffViewMode" label="Layout">
        <DiffLayoutButtons />
      </ControlRow>
      <ControlRow testId="setting-whitespacePolicy" label="Whitespace">
        <WhitespaceMenu named />
      </ControlRow>
      <SettingRow
        testId="setting-softWraps"
        label="Soft wraps"
        description="Break a long line at the edge of the diff; without wraps it scrolls"
        checked={softWraps}
        onCheckedChange={setDiffSoftWraps}
      />
      <SettingRow
        testId="setting-ligatures"
        label="Ligatures"
        description="Draw the ligatures of JetBrains Mono; each keeps the width of its characters"
        checked={ligatures}
        onCheckedChange={setDiffLigatures}
      />
    </div>
  )
}

/** A control that is no switch, after its label */
function ControlRow({ testId, label, children }: { testId: string; label: string; children: ReactNode }) {
  const labelId = useId()
  return (
    <div className="flex min-h-7 items-center gap-3" data-testid={testId}>
      <span id={labelId} className="w-24 shrink-0 text-sm">
        {label}
      </span>
      <div role="group" aria-labelledby={labelId}>
        {children}
      </div>
    </div>
  )
}
