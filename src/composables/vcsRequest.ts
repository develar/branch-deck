import { computed, Ref, shallowRef } from "vue"
import { computedAsync } from "@vueuse/core"
import { commands, Result } from "../bindings"

const defaultBranchPrefixResult: Result<string, string> = { status: "error", error: "Please enter a repository path" }

export function useVcsRequest(repositoryPathRef: Ref<string>) {
  const branchPrefix: Ref<Result<string, string>> = computedAsync(async () => {
    const repositoryPath = repositoryPathRef.value
    return repositoryPath ? await commands.getBranchPrefixFromGitConfig(repositoryPath) : defaultBranchPrefixResult
  }, defaultBranchPrefixResult, { shallow: true })

  const mutableBranchPrefixHolder = shallowRef("")
  const mutableBranchPrefix = computed({
    get() {
      const userValue = mutableBranchPrefixHolder.value
      if (userValue) {
        return userValue
      }
      return branchPrefix.value.status === "ok" ? branchPrefix.value.data : ""
    },
    set(newValue) {
      mutableBranchPrefixHolder.value = newValue.trim()
    },
  })
  return { branchPrefix, mutableBranchPrefix, vcsRequestFactory: new VcsRequestFactory(repositoryPathRef, branchPrefix) }
}

export class VcsRequestFactory {
  constructor(
    private readonly repositoryPath: Ref<string>,
    private readonly branchPrefix: Ref<Result<string, string>>,
  ) {
  }

  createRequest(): VcsRequest {
    return createVcsRequest(this.repositoryPath.value, this.branchPrefix.value)
  }
}

function createVcsRequest(repositoryPath: string, branchPrefixResult: Result<string, string>): VcsRequest {
  if (!repositoryPath.trim()) {
    throw new UserError("Please enter a repository path")
  }
  if (branchPrefixResult.status === "error") {
    throw new UserError(branchPrefixResult.error)
  }

  const branchPrefix = branchPrefixResult.data.trim()
  if (!branchPrefixResult) {
    throw new UserError("Please enter a branch prefix")
  }

  return {
    repositoryPath: repositoryPath.trim(),
    branchPrefix: branchPrefix,
  }
}

export class UserError extends Error {
  constructor(message: string) {
    super(message)
  }
}

export interface VcsRequest {
  readonly repositoryPath: string
  readonly branchPrefix: string
}
