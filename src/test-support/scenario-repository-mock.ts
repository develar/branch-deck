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
export const getSelectedRepositoryPath = (): string => "/repo"
export const isSelectedRepository = (path: string) => path === "/repo"
export const getRepositorySwitchReason = (path: string) => (path === "/repo" ? null : `${path} is no longer the open repository`)
