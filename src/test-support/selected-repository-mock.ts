/** `@/stores/repository` for store tests: `/repo` is selected, its branch prefix is `user` */
export const getSelectedProject = () => ({ path: "/repo" })
export const getDiffRepositoryPath = () => "/repo"
export const getEffectiveBranchPrefix = () => "user"
export const useSelectedProject = () => ({ path: "/repo" })
export const useSelectedProjectPath = () => "/repo"
export const useSelectedProjectField = () => undefined
export const useEffectiveBranchPrefix = () => "user"
export const getFullBranchName = (name: string) => `user/virtual/${name}`
export const getFullStackBranchName = (root: string) => `user/stack/${root}`
