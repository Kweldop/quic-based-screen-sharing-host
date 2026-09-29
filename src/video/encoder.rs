use openh264::{
    encoder::{BitRate, Encoder, EncoderConfig, FrameRate, RateControlMode},
    formats::YUVBuffer,
};

use crate::{AppResult, error::AppError::XCustomMessage};

pub struct VideoEncoder {
    encoder: Encoder,
    pub frame_id: u32,
    width: u32,
    height: u32,
}

impl VideoEncoder {
    pub fn new(width: u32, height: u32) -> AppResult<Self> {
        let config = EncoderConfig::new()
            .max_frame_rate(FrameRate::from_hz(60.0))
            .bitrate(BitRate::from_bps(8_000_000)) // high quality on LAN
            .rate_control_mode(RateControlMode::Quality)
            .skip_frames(true) // encoder skips if falling behind
            .num_threads(0); // 0 = use all CPU cores

        let encoder = Encoder::with_api_config(openh264::OpenH264API::from_source(), config)?;
        Ok(Self {
            encoder,
            frame_id: 0,
            width,
            height,
        })
    }

    pub fn encode(&mut self, yuv: &[u8]) -> AppResult<Vec<u8>> {
        let yuv_buf = YUVBuffer::from_vec(yuv.to_vec(), self.width as usize, self.height as usize);
        let bitstream = self.encoder.encode(&yuv_buf)?;

        let mut nal_bytes = Vec::new();
        for i in 0..bitstream.num_layers() {
            let layer = bitstream.layer(i).ok_or(XCustomMessage("No layer found"))?;
            for nal in 0..layer.nal_count() {
                nal_bytes
                    .extend_from_slice(layer.nal_unit(nal).ok_or(XCustomMessage("No nal found"))?);
            }
        }
        self.frame_id += 1;
        Ok(nal_bytes)
    }
}
