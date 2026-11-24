## MODIFIED Requirements

### Requirement: Bridge: The data of the bridge is the data of the desktop app

The bridge MUST serve the settings, the caches and the journals of the data directory of the app whose binary runs
it, the same as the desktop app and `brd`: those of the backend it joins, or, in its fallback, its own writer of them
beside the app's. The directory MUST come from the identifier compiled into the binary (`app_settings::identity`), so
the app and its bridge key one socket, also in a build of another identifier.

#### Scenario: A setting of the desktop app
- **WHEN** the desktop app saved a setting, and the bridge starts
- **THEN** `bd_bootstrap` gives that setting
  [@test] crates/ide-host/src/process_test.rs (`the_bridge_serves_the_settings_of_the_app_until_its_input_ends`)

#### Scenario: A build of another identifier
- **WHEN** the app is built with `tauri build --config '{"identifier":"org.develar.branch-deck.check"}'`
- **THEN** the app, its bridge and its `brd` use `~/Library/Application Support/org.develar.branch-deck.check` on macOS, and the bridge joins that app's backend
  [@test] crates/app-settings/src/identity_test.rs (`the_identifier_is_the_one_of_the_configuration_under_the_overlay_of_tauri_build`)
  [@test] crates/app-settings/src/identity_test.rs (`the_compiled_identifier_is_the_one_of_tauri_conf_json`)
