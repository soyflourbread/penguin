#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub command: u16,
    pub telemetry: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FrameBuilder {
    frame: Frame,
    inverted: bool,
}

impl FrameBuilder {
    pub fn new(frame: Frame) -> Self {
        Self {
            frame,
            ..Default::default()
        }
    }

    pub fn invert(mut self) -> Self {
        self.inverted = true;
        return self;
    }

    pub fn build(self) -> u16 {
        let frame = self.frame;
        let mut frame = (frame.command << 1) | (frame.telemetry as u16);
        let crc = {
            let mut ret = frame;
            ret ^= frame >> 4;
            ret ^= frame >> 8;
            if self.inverted {
                ret = !ret;
            }
            ret &= 0x0F;
            ret
        };
        frame <<= 4;
        frame |= crc;
        frame
    }
}
