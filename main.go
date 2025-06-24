package main

import (
  "embed"
  "fmt"
  "virtual-branches/backend"

  "github.com/wailsapp/wails/v2"
  "github.com/wailsapp/wails/v2/pkg/options"
  "github.com/wailsapp/wails/v2/pkg/options/assetserver"

  wailsconfigstore "github.com/AndreiTelteu/wails-configstore"
)

//go:embed all:frontend/dist
var assets embed.FS

func main() {
  app := NewApp()

  configStore, err := wailsconfigstore.NewConfigStore("v-branch")
  if err != nil {
    fmt.Printf("could not initialize the config store: %v\n", err)
    return
  }

  err = wails.Run(&options.App{
    Title:  "Virtual Branch Manager",
    Width:  1024,
    Height: 768,
    AssetServer: &assetserver.Options{
      Assets: assets,
    },
    BackgroundColour: &options.RGBA{R: 27, G: 38, B: 54, A: 1},
    OnStartup:        app.OnStartup,
    Bind: []interface{}{
      app,
      configStore,
    },
    EnumBind: []interface{}{
      backend.AllBranchSyncStatuses,
    },
  })

  if err != nil {
    println("Error:", err.Error())
  }
}
