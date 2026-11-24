# Tasks

- [x] `set_unattended` puts the setting back and takes the hook away after a failed save, and after a journal it cannot remove
- [x] `brd install --uninstall` turns the mode off through `set_unattended`
- [x] The lock `unattended.lock` over a switch and over the hook refresh of `brd install`; `SettingsService::flush_and_reread`
- [x] The layout of the settings file of Claude Code kept, its empty containers kept (`EmptyBefore`, `unattended-hook.json`), the hook alone in its entry
- [x] The version of Claude Code read at on, an older one refused
- [x] The menu item follows the setting from Rust; a click with no main window switches through the core
