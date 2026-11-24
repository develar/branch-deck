use super::*;
use pretty_assertions::assert_eq;

fn host() -> HostFile {
  HostFile {
    alias: "studio".to_owned(),
    fingerprint: "ab".repeat(32).parse().unwrap(),
    port: 7742,
    hostname: "develar-mac-studio-1965.local".to_owned(),
    addresses: vec!["192.168.1.24:7742".parse().unwrap()],
    repo: "~/projects/idea-1".to_owned(),
  }
}

/// A host file reads back as it was written, in a private directory, as a private file
#[test]
fn a_host_file_reads_back_as_it_was_written() {
  let dir = tempfile::tempdir().unwrap();
  let config = Config::new(dir.path().join("brd-host"));
  config.write_host(&host()).unwrap();
  assert_eq!(config.read_host("studio").unwrap(), host());
  let text = std::fs::read_to_string(config.host_path("studio")).unwrap();
  assert!(text.contains("\"addresses\": [\n    \"192.168.1.24:7742\"\n  ]"), "{text}");
  {
    use std::os::unix::fs::PermissionsExt;
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!((mode(&config.dir().join("hosts")), mode(&config.host_path("studio"))), (0o700, 0o600));
  }
}

#[test]
fn a_missing_host_file_names_the_setup_command() {
  let dir = tempfile::tempdir().unwrap();
  let refusal = Config::new(dir.path().to_path_buf()).read_host("studio").unwrap_err();
  assert_eq!(refusal.code, Code::NoHost);
  assert!(refusal.message.contains("`brd-host setup studio`"), "{}", refusal.message);
  assert!(
    refusal.message.contains("`brd-host set studio <alias>…` makes a set of hosts"),
    "a name of no set names the set too: {}",
    refusal.message
  );
}

#[test]
fn a_host_file_of_another_alias_or_shape_is_refused() {
  let dir = tempfile::tempdir().unwrap();
  let config = Config::new(dir.path().to_path_buf());
  config.write_host(&host()).unwrap();
  std::fs::copy(config.host_path("studio"), config.host_path("other")).unwrap();
  assert_eq!(config.read_host("other").unwrap_err().code, Code::BadHostFile);
  std::fs::write(config.host_path("studio"), "{}").unwrap();
  assert_eq!(config.read_host("studio").unwrap_err().code, Code::BadHostFile);
}

/// The base of an affinity reads back as it was written. A file that holds no commit is no base
#[test]
fn a_base_reads_back_and_a_bad_file_is_no_base() {
  let dir = tempfile::tempdir().unwrap();
  let config = Config::new(dir.path().to_path_buf());
  let commit = "0123456789abcdef0123456789abcdef01234567";
  assert_eq!(config.base("studio", "checkout-1"), None);
  config.write_base("studio", "checkout-1", commit).unwrap();
  assert_eq!(config.base("studio", "checkout-1").as_deref(), Some(commit));
  assert_eq!(config.base("studio", "checkout-2"), None, "each affinity has its own base");
  assert_eq!(config.base("other", "checkout-1"), None, "each host has its own base");
  std::fs::write(dir.path().join("bases/studio/checkout-1"), "master\n").unwrap();
  assert_eq!(config.base("studio", "checkout-1"), None);
  assert_eq!(config.write_base("studio", "../x", commit).unwrap_err().code, Code::Usage);
}

#[test]
fn an_alias_that_cannot_name_a_file_is_refused() {
  for alias in ["", "../x", ".hidden", "a b"] {
    assert_eq!(check_alias(alias).unwrap_err().code, Code::Usage, "{alias:?}");
  }
  check_alias("studio-2.local").unwrap();
}

/// Two writers at the same time never tear the file: each reader sees one whole host file, and no temporary file stays
#[test]
fn concurrent_writers_leave_one_whole_host_file() {
  let dir = tempfile::tempdir().unwrap();
  let config = Config::new(dir.path().to_path_buf());
  config.write_host(&host()).unwrap();
  std::thread::scope(|scope| {
    for writer in 0..8u16 {
      let config = &config;
      scope.spawn(move || {
        for round in 0..20u16 {
          let mut file = host();
          file.port = 7000 + writer * 100 + round;
          config.write_host(&file).unwrap();
          config.read_host("studio").unwrap();
        }
      });
    }
  });
  let names: Vec<String> = std::fs::read_dir(dir.path().join("hosts"))
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .collect();
  assert_eq!(names, ["studio.json"]);
}

/// A set of hosts reads back as it was written, beside the host files: its name is no alias of a host file, each member
/// has one, once, and a name with a host file names that host first
#[test]
fn a_set_of_hosts_names_its_members_and_never_an_alias() {
  let dir = tempfile::tempdir().unwrap();
  let config = Config::new(dir.path().join("brd-host"));
  config.write_host(&host()).unwrap();
  config
    .write_host(&HostFile {
      alias: "local".to_owned(),
      ..host()
    })
    .unwrap();
  let member = |alias: &str, idle_only| SetMember {
    alias: alias.to_owned(),
    idle_only,
  };
  let set = HostSet {
    name: "any".to_owned(),
    members: vec![member("studio", false), member("local", true)],
  };
  config.write_set(&set).unwrap();
  assert_eq!(config.read_set("any").unwrap(), Some(set.clone()));
  assert_eq!(config.set_of("any").unwrap(), Some(set.clone()));
  assert_eq!(
    (config.set_of("studio").unwrap(), config.set_of("other").unwrap()),
    (None, None),
    "a host file first, and no set"
  );
  let text = std::fs::read_to_string(config.set_path("any")).unwrap();
  assert_eq!(
    text,
    "{\n  \"name\": \"any\",\n  \"members\": [\n    {\n      \"alias\": \"studio\"\n    },\n    {\n      \"alias\": \"local\",\n      \"idle_only\": true\n    }\n  ]\n}\n"
  );
  assert_eq!(config.aliases(), ["local", "studio"], "a set is no host");

  let refused = |set: HostSet| config.write_set(&set).unwrap_err();
  let named_as_host = refused(HostSet {
    name: "studio".to_owned(),
    ..set.clone()
  });
  assert_eq!(named_as_host.code, Code::Usage);
  assert!(named_as_host.message.starts_with("studio is the alias of a host file"), "{}", named_as_host.message);
  let twice = refused(HostSet {
    members: vec![member("studio", false), member("studio", true)],
    ..set.clone()
  });
  assert_eq!((twice.code, twice.message.as_str()), (Code::Usage, "the set any names studio twice"));
  let unknown = refused(HostSet {
    members: vec![member("mini", false)],
    ..set.clone()
  });
  assert_eq!(unknown.code, Code::NoHost);
  let empty = refused(HostSet { members: Vec::new(), ..set });
  assert_eq!((empty.code, empty.message.as_str()), (Code::Usage, "the set any needs a member"));
  std::fs::write(config.set_path("broken"), "{\"name\": \"other\", \"members\": []}").unwrap();
  assert_eq!(config.read_set("broken").unwrap_err().code, Code::BadHostFile);
}
