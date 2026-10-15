use crate::text_input::decode_text;
use pretty_assertions::assert_eq;

#[test]
fn a_bom_is_dropped_and_utf16_is_decoded() {
  assert_eq!(decode_text("{\"a\":\"é\"}".as_bytes()), Ok("{\"a\":\"é\"}".to_string()));
  assert_eq!(decode_text(b"\xEF\xBB\xBF{}"), Ok("{}".to_string()));
  let le: Vec<u8> = [0xFF, 0xFE].into_iter().chain("{é}".encode_utf16().flat_map(u16::to_le_bytes)).collect();
  assert_eq!(decode_text(&le), Ok("{é}".to_string()));
  let be: Vec<u8> = [0xFE, 0xFF].into_iter().chain("{é}".encode_utf16().flat_map(u16::to_be_bytes)).collect();
  assert_eq!(decode_text(&be), Ok("{é}".to_string()));
  assert!(decode_text(b"\xFF\xFE{").is_err());
  assert!(decode_text(b"\xC3").is_err());
}
