/** `@/stores/repository` for store tests: `/repo` is selected, its branch prefix is `user` */
export const getSelectedProject = () => ({ path: "/repo" })
export const getEffectiveBranchPrefix = () => "user"
export const useSelectedProject = () => ({ path: "/repo" })
export const useEffectiveBranchPrefix = () => "user"
export const getFullBranchName = (name: string) => `user/virtual/${name}`
