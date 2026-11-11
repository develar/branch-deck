// the entry of the bundle of the page mock (`tauriMockInitScript`): it runs in the page before its scripts
import { PAGE_COMMANDS, tauriMockScript } from "./tauri-mocks"

tauriMockScript(PAGE_COMMANDS)
