use std::io::Cursor;

use media_codec_bitstream::{BigEndian, BitReader};

#[inline]
pub fn from_slice_be<'a>(data: &'a [u8]) -> BitReader<Cursor<&'a [u8]>, BigEndian> {
    BitReader::new(Cursor::new(data))
}

#[test]
fn test_read_ue() {
    // Test vectors: [bits, expected_value]
    let test_cases: &[(&[u8], u32)] = &[
        (&[0b10000000], 0),  // 1 -> 0
        (&[0b01000000], 1),  // 010 -> 1
        (&[0b01100000], 2),  // 011 -> 2
        (&[0b00100000], 3),  // 00100 -> 3
        (&[0b00101000], 4),  // 00101 -> 4
        (&[0b00110000], 5),  // 00110 -> 5
        (&[0b00111000], 6),  // 00111 -> 6
        (&[0b00010000], 7),  // 0001000 -> 7
        (&[0b00010010], 8),  // 0001001 -> 8
        (&[0b00010100], 9),  // 0001010 -> 9
        (&[0b00010110], 10), // 0001011 -> 10
        (&[0b00011000], 11), // 0001100 -> 11
        (&[0b00011010], 12), // 0001101 -> 12
        (&[0b00011100], 13), // 0001110 -> 13
        (&[0b00011110], 14), // 0001111 -> 14
    ];

    for (data, expected) in test_cases {
        let mut reader = from_slice_be(data);
        let value = reader.read_ue().unwrap();
        assert_eq!(value, *expected, "data: {:08b}, expected: {}", data[0], expected);
    }
}

#[test]
fn test_read_se() {
    // Test vectors: [ue_value, se_value]
    // 0 -> 0, 1 -> 1, 2 -> -1, 3 -> 2, 4 -> -2, 5 -> 3, 6 -> -3
    let test_cases: &[(&[u8], i32)] = &[
        (&[0b10000000], 0),  // ue(0) -> se(0)
        (&[0b01000000], 1),  // ue(1) -> se(1)
        (&[0b01100000], -1), // ue(2) -> se(-1)
        (&[0b00100000], 2),  // ue(3) -> se(2)
        (&[0b00101000], -2), // ue(4) -> se(-2)
        (&[0b00110000], 3),  // ue(5) -> se(3)
        (&[0b00111000], -3), // ue(6) -> se(-3)
    ];

    for (data, expected) in test_cases {
        let mut reader = from_slice_be(data);
        let value = reader.read_se().unwrap();
        assert_eq!(value, *expected, "data: {:08b}, expected: {}", data[0], expected);
    }
}

#[test]
fn test_peek_bits() {
    let data = [0b10110001, 0b01010101];
    let mut reader = from_slice_be(&data);

    // Peek should not consume bits
    assert_eq!(reader.peek::<4, u8>().unwrap(), 0b1011);
    assert_eq!(reader.peek::<4, u8>().unwrap(), 0b1011);
    assert_eq!(reader.peek::<8, u8>().unwrap(), 0b10110001);

    // Now actually read
    assert_eq!(reader.read::<4, u8>().unwrap(), 0b1011);
    assert_eq!(reader.peek::<4, u8>().unwrap(), 0b0001);
}

#[test]
fn test_peek_ue() {
    let data = [0b01000000]; // ue(1)
    let mut reader = from_slice_be(&data);

    // Peek should not consume bits
    assert_eq!(reader.peek_ue().unwrap(), 1);
    assert_eq!(reader.peek_ue().unwrap(), 1);

    // Now actually read
    assert_eq!(reader.read_ue().unwrap(), 1);
}

#[test]
fn test_peek_se() {
    let data = [0b01100000]; // ue(2) -> se(-1)
    let mut reader = from_slice_be(&data);

    // Peek should not consume bits
    assert_eq!(reader.peek_se().unwrap(), -1);
    assert_eq!(reader.peek_se().unwrap(), -1);

    // Now actually read
    assert_eq!(reader.read_se().unwrap(), -1);
}

#[test]
fn test_peek_var() {
    let data = [0b10110001, 0b01010101];
    let mut reader = from_slice_be(&data);

    assert_eq!(reader.peek_var::<u8>(4).unwrap(), 0b1011);
    assert_eq!(reader.peek_var::<u8>(4).unwrap(), 0b1011);
    assert_eq!(reader.read_var::<u8>(4).unwrap(), 0b1011);
    assert_eq!(reader.peek_var::<u8>(4).unwrap(), 0b0001);
}

#[test]
fn test_peek_bit() {
    let data = [0b10110001];
    let mut reader = from_slice_be(&data);

    assert!(reader.peek_bit().unwrap());
    assert!(reader.peek_bit().unwrap());
    assert!(reader.read_bit().unwrap());
    assert!(!reader.peek_bit().unwrap());
}
