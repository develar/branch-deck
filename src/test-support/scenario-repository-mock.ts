/** `@/stores/repository` for the scenario cases: `/repo` is selected, its branch prefix is `test-user` (the README of the cases) */
export const getSelectedProject = () => ({ path: "/repo" })
export const getDiffRepositoryPath = () => "/repo"
export const getEffectiveBranchPrefix = () => "test-user"
export const useSelectedProject = () => ({ path: "/repo" })
export const useSelectedProjectPath = () => "/repo"
export const useSelectedProjectField = () => undefined
export const useEffectiveBranchPrefix = () => "test-user"
export const getFullBranchName = (name: string) => `test-user/virtual/${name}`
export const getFullStackBranchName = (root: string) => `test-user/stack/${root}`
export const getLoadingPromise = () => null
