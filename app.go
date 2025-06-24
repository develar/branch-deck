package main

import (
  "context"
  "fmt"
  "sync"
  "virtual-branches/backend"

  "github.com/wailsapp/wails/v2/pkg/runtime"
)

// App struct
type App struct {
  gitInfo backend.GitInfo
  ctx     context.Context

  gitInfoMutex sync.Mutex
}

// NewApp creates a new App application struct
func NewApp() *App {
  return &App{}
}

func (t *App) getGitInfo() (backend.GitInfo, error) {
  t.gitInfoMutex.Lock()
  defer t.gitInfoMutex.Unlock()

  if !t.gitInfo.IsZero() {
    return t.gitInfo, nil
  }

  result, err := backend.NewGitInfo()
  if err != nil {
    return backend.ZeroGitInfo, err
  }
  return result, nil
}

func withGitInfo[T any](app *App, f func(gitInfo backend.GitInfo) T) (T, error) {
  gitInfo, err := app.getGitInfo()
  if err != nil {
    var zero T
    return zero, fmt.Errorf("failed to get git info: %w", err)
  }

  return f(gitInfo), nil
}

// OnStartup is called when the app starts
func (t *App) OnStartup(ctx context.Context) {
  t.ctx = ctx
}

//goland:noinspection GrazieInspection
func (t *App) OpenDirectoryDialog() string {
  dialog, _ := runtime.OpenDirectoryDialog(t.ctx, runtime.OpenDialogOptions{})
  return dialog
}

func (t *App) GetBranchPrefixFromGitConf(repositoryPath string) backend.GlobalBranchPrefix {
  result, err := withGitInfo(t, func(gitInfo backend.GitInfo) backend.GlobalBranchPrefix {
    value, err := backend.GetBranchPrefixFromGitConf(gitInfo.Path, repositoryPath)
    if err != nil {
      return backend.GlobalBranchPrefix{
        Error: fmt.Sprintf("Failed to get branch prefix (git version: %s): %v", gitInfo.Version, err),
      }
    }

    return backend.GlobalBranchPrefix{
      BranchPrefix: value,
    }
  })

  if err != nil {
    return backend.GlobalBranchPrefix{
      Error: err.Error(),
    }
  }

  return result
}

// CreateVirtualBranches processes the repository and creates virtual branches
func (t *App) CreateVirtualBranches(request backend.VcsRequest) backend.ActionResult {
  err := request.Validate()
  if err != nil {
    return backend.ActionResult{
      Success: false,
      Message: err.Error(),
    }
  }

  result, err := withGitInfo(t, func(gitInfo backend.GitInfo) backend.ActionResult {
    branches, err := backend.CreateBranches(request.BranchPrefix, backend.NewGit(request.RepositoryPath, gitInfo.Path))
    if err != nil {
      return backend.ActionResult{
        Success: false,
        Message: fmt.Sprintf("Failed to create branches: %v", err),
      }
    }

    return backend.ActionResult{
      Success:  true,
      Branches: branches,
    }
  })

  if err != nil {
    return backend.ActionResult{
      Success: false,
      Message: err.Error(),
    }
  }

  return result
}

// PushBranch pushes a specific branch to the remote repository
func (t *App) PushBranch(request backend.VcsRequest, branchName string) backend.ActionResult {
  return backend.PushBranchToRemote(request.RepositoryPath, request.BranchPrefix, branchName)
}
