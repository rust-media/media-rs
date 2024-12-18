#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaFrameType {
    Audio = 0,
    Video,
    Data,
}
