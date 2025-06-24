package backend

import (
  "os/exec"
  "strings"
)

type GitInfo struct {
  Version string
  Path    string
}

var ZeroGitInfo GitInfo

func (g *GitInfo) IsZero() bool {
  return g.Path == ""
}

func NewGitInfo() (GitInfo, error) {
  gitPath, err := readString(exec.Command("/bin/zsh", "-l", "-c", "which git"))
  if err != nil {
    return ZeroGitInfo, err
  }

  gitVersion, err := readString(exec.Command(gitPath, "--version"))
  if err != nil {
    return ZeroGitInfo, err
  }
  return GitInfo{
    Version: gitVersion,
    Path:    gitPath,
  }, nil
}

func readString(cmd *exec.Cmd) (string, error) {
  output, err := cmd.CombinedOutput()
  if err != nil {
    return "", err
  }
  outputStr := strings.TrimSpace(string(output))
  return outputStr, nil
}
