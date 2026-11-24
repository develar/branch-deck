# Tasks

- [x] `app_settings::claude_hook` (moved from bd-cli): `add_hook`, `remove_hook`, `has_hook`
- [x] `set_unattended` adds the hook before a switch on and removes it after a switch off; `AgentCliHost::unattended_hook`
- [x] `brd install` keeps the hook as the mode has it; `brd unattended` answers `hook: present|absent` and `warning`
- [x] The hook states of Settings → Tools and the install offer of the switch-on toast removed; the warning of a switch off
