/** `@/stores/repository` for store tests: `/repo` is selected, its branch prefix is `user` */
export const getSelectedProject = () => ({ path: "/repo" })
export const getDiffRepositoryPath = () => "/repo"
export const getSelectedRepositoryPath = () => "/repo"
export const isSelectedRepository = (path: string) => path === "/repo"
export const getRepositorySwitchReason = (path: string) => (path === "/repo" ? null : `${path} is no longer the open repository`)
export const getEffectiveBranchPrefix = () => "user"
export const useSelectedProject = () => ({ path: "/repo" })
export const useSelectedProjectPath = () => "/repo"
export const useSelectedProjectField = () => undefined
export const useEffectiveBranchPrefix = () => "user"
export const getFullBranchName = (name: string) => `user/virtual/${name}`
export const getFullStackBranchName = (root: string) => `user/stack/${root}`
