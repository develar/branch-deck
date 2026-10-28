use super::*;

/// Ctrl+C ends the upload at the next chunk
#[test]
fn an_upload_stops_at_ctrl_c() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("pack");
  std::fs::write(&path, b"abc").unwrap();
  let mut calm = Upload {
    file: File::open(&path).unwrap(),
    stop: &|| false,
  };
  let mut read = Vec::new();
  calm.read_to_end(&mut read).unwrap();
  assert_eq!(read, b"abc");
  let mut stopped = Upload {
    file: File::open(&path).unwrap(),
    stop: &|| true,
  };
  stopped.read_to_end(&mut read).unwrap_err();
}

/// A host without any of the candidates gets no pack
#[test]
fn no_candidate_is_behind() {
  let refusal = plan_pack("studio", &[], Some(70)).err().unwrap();
  assert_eq!(refusal.code, Code::Behind);
  assert!(refusal.message.contains("the whole history of 70 commits"), "{}", refusal.message);
  plan_pack("studio", &["c".to_owned()], None).unwrap();
}
