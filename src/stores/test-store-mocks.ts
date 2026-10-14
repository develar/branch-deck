/** Module mocks for tests of `@/stores/branch-sync` that set the store state directly; load them in `vi.mock` factories */
export const repositoryMock = {
  getLoadingPromise: () => null,
  getSelectedProject: () => null,
  useSelectedProject: () => null,
  vcsRequestFactory: { createRequest: () => ({}) },
}

export const repositorySettingsMock = { updateProject: () => {} }

export const archivedBranchesMock = { updateFromArchivedNames: () => {}, updateFromIntegrationInfo: () => {} }
