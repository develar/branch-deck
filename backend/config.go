package backend

import (
  "os"
  "os/exec"
  "strings"
)

func GetBranchPrefixFromGitConf(gitPath string, repositoryPath string) (string, error) {
  cmd := exec.Command(gitPath, "config", "get", "v-branch.branchPrefix")
  if isDir(repositoryPath) {
    cmd.Dir = repositoryPath
  }
  output, err := cmd.CombinedOutput()
  outputStr := strings.TrimSpace(string(output))
  if err != nil {
    return "", err
  }
  return strings.TrimSpace(outputStr), nil
}

func isDir(path string) bool {
  if path == "" {
    return false
  }

  info, err := os.Stat(path)
  if err == nil {
    return info.IsDir()
  }
  return false
}
