/** Module mocks for tests of `@/stores/branch-sync` that set the store state directly; load them in `vi.mock` factories */
export const repositoryMock = {
  getSelectedProject: () => null,
  getSelectedRepositoryPath: () => "",
  useSelectedProject: () => null,
  vcsRequestFactory: { createRequest: () => ({}) },
}

export const repositorySettingsMock = { updateProject: () => {} }

export const archivedBranchesMock = {
  updateFromArchivedNames: () => {},
  updateFromIntegrationInfo: () => {},
  updateFromIntegrationInfos: () => {},
  clearNewlyArchived: () => {},
  settleArchivedPlaceholders: () => {},
}

/** `@/stores/activity` for tests of the modules that record what they did: nothing is recorded */
export const activityMock = {
  UNDO_WINDOW_MS: 10_000,
  recordActivity: () => Promise.resolve(null),
  recordLanded: () => {},
  recordReviewRequest: () => {},
  recordRewrite: (_repositoryPath: string, _title: string, undo: () => Promise<boolean>) => () => void undo(),
  recordRobotNews: () => {},
  recordUpdate: () => {},
  refreshActivity: () => Promise.resolve(),
}
