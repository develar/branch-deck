use crate::private_file::read_private;
use crate::secret_store::SecretStore;

#[test]
fn without_the_keychain_a_secret_is_the_0600_file() {
  let dir = tempfile::tempdir().unwrap();
  // the tests never turn the keychain on (`enable_keychain`)
  let store = SecretStore::of(&dir.path().join("org.example.app").join("token.json"));
  assert_eq!(store.keychain, None);
  store.write("{\"token\": \"secret\"}").unwrap();
  assert_eq!(read_private(&store.file, "the token file").as_deref(), Some("{\"token\": \"secret\"}"));
  store.remove().unwrap();
  assert_eq!(store.read("the token file"), None);
}

/// The keychain through a `security` of the tests: a shell script, so on unix only
#[cfg(unix)]
mod keychain {
  use crate::private_file::{read_private, write_private};
  use crate::secret_store::{Keychain, SecretStore};
  use std::path::Path;

  /// A `security` of the tests: items as files beside it, `-i` reads one command from stdin, `FAIL` in the directory
  /// makes every call fail as a locked keychain does
  fn fake_security(dir: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("security");
    std::fs::write(
      &script,
      r#"#!/bin/sh
  here=$(dirname "$0")
  [ -f "$here/FAIL" ] && { echo "The user name or passphrase you entered is not correct." >&2; exit 51; }
  if [ "$1" = "-i" ]; then read -r line; set -- $line; fi
  echo "$*" >> "$here/calls"
  command=$1; shift
  while [ $# -gt 0 ]; do
    case $1 in
      -s) service=$2; shift 2 ;;
      -a) account=$2; shift 2 ;;
      -w) secret=${2-}; shift; [ $# -gt 0 ] && shift ;;
      *) shift ;;
    esac
  done
  item="$here/item.$service.$account"
  case $command in
    add-generic-password) printf %s "$secret" > "$item" ;;
    find-generic-password) [ -f "$item" ] || exit 44; cat "$item"; echo ;;
    delete-generic-password) [ -f "$item" ] || exit 44; rm "$item" ;;
  esac
  "#,
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
  }

  fn store_with_keychain(dir: &Path) -> SecretStore {
    SecretStore {
      file: dir.join("org.example.app").join("token.json"),
      keychain: Some(Keychain {
        security: fake_security(dir),
        service: "org.example.app".to_string(),
        account: "token".to_string(),
      }),
    }
  }

  #[test]
  fn a_secret_lives_in_the_keychain_through_stdin_and_hex_without_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_with_keychain(dir.path());
    let secret = "{\"token\": \"perm-secret\", \"full_name\": \"Ivan Šachov\"}";
    store.write(secret).unwrap();
    assert!(!store.file.exists());
    assert_eq!(store.read("the token"), Some(secret.to_string()));
    // the secret is never an argument: `-i` took the command from stdin, as hex
    let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
    assert!(!calls.contains("perm-secret"), "{calls}");
    assert!(calls.starts_with("add-generic-password -U -s org.example.app -a token -w 7b22"), "{calls}");

    store.remove().unwrap();
    assert_eq!(store.read("the token"), None);
    store.remove().unwrap();
  }

  #[test]
  fn the_file_of_an_earlier_version_is_copied_into_the_keychain_and_removed_by_the_next_write() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_with_keychain(dir.path());
    write_private(&store.file, b"{\"token\": \"old\"}").unwrap();
    assert_eq!(store.read("the token").as_deref(), Some("{\"token\": \"old\"}"));
    assert_eq!(store.keychain.as_ref().unwrap().find(), Ok(Some("{\"token\": \"old\"}".to_string())));
    // an installed earlier version still reads the file
    assert!(store.file.exists());
    store.write("{\"token\": \"new\"}").unwrap();
    assert!(!store.file.exists());
    assert_eq!(store.read("the token").as_deref(), Some("{\"token\": \"new\"}"));
  }

  #[test]
  fn a_locked_keychain_leaves_the_secret_in_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_with_keychain(dir.path());
    std::fs::write(dir.path().join("FAIL"), "").unwrap();
    store.write("{\"token\": \"kept\"}").unwrap();
    assert_eq!(read_private(&store.file, "the token file").as_deref(), Some("{\"token\": \"kept\"}"));
    assert_eq!(store.read("the token").as_deref(), Some("{\"token\": \"kept\"}"));
  }
}
