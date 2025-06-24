import {backend} from "../../wailsjs/go/models"
import {GetBranchPrefixFromGitConf} from "../../wailsjs/go/main/App"
import {Ref} from "vue"
import {computedAsync} from '@vueuse/core'

export async function useVcsRequest(repositoryPath: Ref<string>) {
  const branchPrefix = computedAsync(async () => {
    const result = await GetBranchPrefixFromGitConf(repositoryPath.value)
    console.log("GetBranchPrefixFromGitConf", result)
    return result.branchPrefix
  })
  return {branchPrefix, vcsRequestFactory: new VcsRequestFactory(repositoryPath, branchPrefix)}
}

export class VcsRequestFactory {
  constructor(
    private readonly repositoryPath: Ref<string>,
    private readonly branchPrefix: Ref<string>,
  ) {
  }

  createRequest(): backend.VcsRequest {
    return createVcsRequest(this.repositoryPath.value, this.branchPrefix.value)
  }
}

function createVcsRequest(repositoryPath: string, branchPrefix: string): backend.VcsRequest {
  if (!repositoryPath.trim()) {
    throw new UserError("Please enter a repository path")
  }
  if (!branchPrefix.trim()) {
    throw new UserError("Please enter a branch prefix")
  }

  return {
    RepositoryPath: repositoryPath.trim(),
    BranchPrefix: branchPrefix.trim(),
  }
}

export class UserError extends Error {
  constructor(message: string) {
    super(message)
  }
}
