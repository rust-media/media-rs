use media_core::circular_buffer::CircularBuffer;

#[test]
fn test_read_write() {
    let mut buf = CircularBuffer::new(16);
    let input = b"hello world";
    let mut output = [0u8; 11];

    assert_eq!(buf.write(input).unwrap(), input.len());
    assert_eq!(buf.len(), input.len());

    assert_eq!(buf.read(&mut output).unwrap(), input.len());
    assert_eq!(&output, input);
}

#[test]
fn test_wrap_around_read_write() {
    let mut buf = CircularBuffer::new(6);
    buf.write(b"012345").unwrap();

    let mut output = [0u8; 3];
    buf.read(&mut output).unwrap();

    buf.write(b"abc").unwrap();

    let mut output = [0u8; 6];
    assert_eq!(buf.read(&mut output).unwrap(), 6);
    assert_eq!(&output, b"345abc");
}

#[test]
fn test_peek() {
    let mut buf = CircularBuffer::new(16);
    buf.write(b"hello world").unwrap();

    let mut output = [0u8; 11];
    buf.peek(&mut output).unwrap();
    assert_eq!(&output, b"hello world");
    assert_eq!(buf.len(), 11);
}

#[test]
fn test_consume() {
    let mut buf = CircularBuffer::new(16);
    buf.write(b"hello world").unwrap();

    let consumed = buf.consume(6);
    assert_eq!(consumed, 6);
    assert_eq!(buf.len(), 5);

    let mut output = [0u8; 5];
    buf.read(&mut output).unwrap();
    assert_eq!(&output, b"world");
}

#[test]
fn test_auto_grow() {
    let mut buf = CircularBuffer::new(8);
    buf.write(b"0123456789").unwrap();
    assert_eq!(buf.len(), 10);
    assert!(buf.capacity() >= 16);
}
