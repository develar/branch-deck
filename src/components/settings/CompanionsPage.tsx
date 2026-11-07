import { CompanionsSetting } from "@/components/settings/CompanionsSetting"
import { ConfigReadError, SettingsSection } from "@/components/settings/SettingRow"
import { InlineCode } from "@/components/shared/InlineCode"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { useRepositoryConfig } from "@/hooks/useRepositoryConfig"
import { useSelectedProjectPath } from "@/stores/repository"

/**
 * Settings → Companions (spec/branch/declared-needs.spec.md#companion-repositories): the repositories whose branches
 * land in an order with the selected one, which a need names as `companion:branch`; written to `.git/config` of both
 */
export function CompanionsPage() {
  const path = useSelectedProjectPath()
  if (path === undefined) {
    return (
      <p className="text-sm text-muted-foreground" data-testid="companions-no-repository">
        Select a repository to see its companions
      </p>
    )
  }
  return <RepositoryCompanions key={path} path={path} />
}

function RepositoryCompanions({ path }: { path: string }) {
  const { state, retry, showWritten } = useRepositoryConfig(path)
  return (
    <SettingsSection
      title="Companions"
      description={
        <>
          {"Repositories whose branches land in an order with this one, in the app and bd · "}
          <InlineCode>branchdeck.companion.‹name›.path</InlineCode>
        </>
      }
    >
      {state.status === "loading" && <LoadingLine>Reading the settings…</LoadingLine>}
      {state.status === "error" && <ConfigReadError testId="setting-companions-error" message={state.message} onRetry={retry} />}
      {state.status === "loaded" && <CompanionsSetting path={path} config={state.config} showWritten={showWritten} />}
    </SettingsSection>
  )
}
