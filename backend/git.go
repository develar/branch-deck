package backend

import (
  "fmt"
  "os/exec"
  "strings"
)

type Git struct {
  repoPath string
}

func NewGit(repoPath string) *Git {
  return &Git{repoPath: repoPath}
}

func (g *Git) execCommand(args ...string) (string, error) {
  cmd := exec.Command("git", args...)
  cmd.Dir = g.repoPath
  output, err := cmd.CombinedOutput()
  if err != nil {
    return "", fmt.Errorf("git command failed: %v: %s", err, output)
  }
  return strings.TrimSpace(string(output)), nil
}

func (g *Git) execCommandNoOutput(args ...string) error {
  cmd := exec.Command("git", args...)
  cmd.Dir = g.repoPath
  err := cmd.Run()
  if err != nil {
    return fmt.Errorf("git command failed: %v", err)
  }
  return nil
}

func (g *Git) AddNote(message string, commitHash string) error {
  return g.execCommandNoOutput("notes", "add", "-f", "-m", message, commitHash)
}

//func (g *Git) GetNote(commitHash string) (string, error) {
//  return g.execCommand("notes", "show", commitHash)
//}

//func (g *Git) HasRemote(name string) (bool, error) {
//  output, err := g.execCommand("remote")
//  if err != nil {
//    return false, err
//  }
//  remotes := strings.Split(output, "\n")
//  for _, remote := range remotes {
//    if remote == name {
//      return true, nil
//    }
//  }
//  return false, nil
//}
