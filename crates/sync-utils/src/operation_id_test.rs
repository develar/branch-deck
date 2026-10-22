use crate::operation_id::OperationId;

#[test]
fn writes_the_ulid_of_the_spec() {
  // the example of the ULID spec: 01ARZ3NDEK is 1469922850259 ms, the rest its 80 random bits
  let id = OperationId::from_parts(1_469_922_850_259, 0xd676_4c61_efb9_9302_bd5b);
  assert_eq!(id.to_string(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
  assert_eq!(id.timestamp_ms(), 1_469_922_850_259);
  assert_eq!(serde_json::to_string(&id).unwrap(), r#""01ARZ3NDEKTSV4RRFFQ69G5FAV""#);
}

#[test]
fn a_new_id_starts_with_now_and_differs_from_the_next() {
  let before = jiff::Timestamp::now().as_millisecond().cast_unsigned();
  let first = OperationId::new();
  let second = OperationId::new();
  let after = jiff::Timestamp::now().as_millisecond().cast_unsigned();
  assert!((before..=after).contains(&first.timestamp_ms()), "{} not in {before}..={after}", first.timestamp_ms());
  assert_ne!(first, second);
  assert_eq!(first.to_string().len(), 26);
  assert!(first.to_string().bytes().all(|byte| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&byte)), "{first}");
}

#[test]
fn ids_sort_by_their_time() {
  let earlier = OperationId::from_parts(1000, u128::MAX);
  let later = OperationId::from_parts(1001, 0);
  assert!(earlier < later);
  assert!(earlier.to_string() < later.to_string());
}
