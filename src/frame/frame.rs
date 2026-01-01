use super::frame_header::FrameHeader;
use super::subframe::Subframe;

pub struct Frame {
    pub header: FrameHeader,
    pub subframes: Vec<Subframe>,
}
