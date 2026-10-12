import { cn } from "cn"
import { FieldGroup } from "@/components/layout/FieldGroup"
import { Input } from "@/components/ui/input"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { BranchPrefixHelp } from "@/components/repository/BranchPrefixHelp"
import { useEffectiveBranchPrefix, useRepository } from "@/stores/repository"
import { setGlobalUserBranchPrefix, useAppSettings } from "@/stores/settings/appSettings"

interface BranchPrefixFieldProps {
  disabled?: boolean
}

export function BranchPrefixField({ disabled = false }: BranchPrefixFieldProps) {
  const globalUserBranchPrefix = useAppSettings(state => state.globalUserBranchPrefix)
  const gitProvidedBranchPrefix = useRepository(state => state.gitProvidedBranchPrefix)
  const isLoadingBranchPrefix = useRepository(state => state.isLoadingBranchPrefix)
  const effectiveBranchPrefix = useEffectiveBranchPrefix()

  // show the git-provided prefix as a hint when the user hasn't entered anything
  const placeholder = gitProvidedBranchPrefix.status === "ok" && gitProvidedBranchPrefix.data
    ? gitProvidedBranchPrefix.data
    : "Set prefix..."
  // don't warn while loading
  const configured = isLoadingBranchPrefix || effectiveBranchPrefix !== ""
  const warn = !configured && !disabled

  return (
    <Tooltip>
      <TooltipTrigger render={<FieldGroup />}>
        <Input
          value={globalUserBranchPrefix ?? ""}
          onChange={event => setGlobalUserBranchPrefix(event.target.value || undefined)}
          disabled={disabled}
          placeholder={placeholder}
          aria-label="Branch prefix"
          className={cn("h-7 w-28 border-border bg-background md:text-xs", warn && "border-warning focus-visible:border-warning focus-visible:ring-warning/30")}
        />
        <BranchPrefixHelp configured={configured} disabled={disabled} />
      </TooltipTrigger>
      <TooltipContent>Your personal prefix (e.g., username) prepended to all branch names</TooltipContent>
    </Tooltip>
  )
}
