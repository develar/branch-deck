/** Module mocks for tests of `@/stores/branch-sync` that set the store state directly; load them in `vi.mock` factories */
export const repositoryMock = {
  getLoadingPromise: () => null,
  getSelectedProject: () => null,
  useSelectedProject: () => null,
  vcsRequestFactory: { createRequest: () => ({}) },
}

export const repositorySettingsMock = { updateProject: () => {} }

export const archivedBranchesMock = { updateFromArchivedNames: () => {}, updateFromIntegrationInfo: () => {}, clearNewlyArchived: () => {} }

/** `@/stores/activity` for tests of the modules that record what they did: nothing is recorded */
export const activityMock = {
  UNDO_WINDOW_MS: 10_000,
  recordActivity: () => null,
  recordLanded: () => {},
  recordPush: () => {},
  recordRewrite: (_title: string, undo: () => Promise<void>) => () => void undo(),
  recordRobot: () => {},
  recordUpdate: () => {},
}
