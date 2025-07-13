import type { Ref} from "vue";
import { computed, shallowRef } from "vue"
import { computedAsync } from "@vueuse/core"
import { commands } from "~/utils/bindings"
import type { Result } from "~/utils/bindings"

const defaultBranchPrefixResult: Result<string, string> = { status: "error", error: "Please enter a repository path" }

export function useVcsRequest(repositoryPathRef: Ref<string>) {
  const gitProvidedBranchPrefix: Ref<Result<string, string>> = computedAsync(async () => {
    const repositoryPath = repositoryPathRef.value
    return repositoryPath ? await commands.getBranchPrefixFromGitConfig(repositoryPath) : defaultBranchPrefixResult
  }, defaultBranchPrefixResult, { shallow: true })

  const mutableBranchPrefixHolder = shallowRef("")
  const mutableBranchPrefix = computed({
    get() {
      const userValue = mutableBranchPrefixHolder.value?.trim()
      if (userValue) {
        return userValue
      }
      return gitProvidedBranchPrefix.value.status === "ok" ? gitProvidedBranchPrefix.value.data : ""
    },
    set(newValue) {
      mutableBranchPrefixHolder.value = newValue.trim()
    },
  })
  return { gitProvidedBranchPrefix, mutableBranchPrefix, vcsRequestFactory: new VcsRequestFactory(repositoryPathRef, mutableBranchPrefix) }
}

export class VcsRequestFactory {
  constructor(
    private readonly repositoryPath: Ref<string>,
    private readonly branchPrefix: Ref<string>,
  ) {
  }

  createRequest(): VcsRequest {
    return createVcsRequest(this.repositoryPath.value, this.branchPrefix.value)
  }
}

function createVcsRequest(repositoryPath: string, branchPrefix: string): VcsRequest {
  if (!repositoryPath.trim()) {
    throw new UserError("Please enter a repository path")
  }
  if (!branchPrefix) {
    throw new UserError("Please configure a branch prefix")
  }

  return {
    repositoryPath: repositoryPath.trim(),
    branchPrefix: branchPrefix,
  }
}

export class UserError extends Error {}

export interface VcsRequest {
  readonly repositoryPath: string
  readonly branchPrefix: string
}
