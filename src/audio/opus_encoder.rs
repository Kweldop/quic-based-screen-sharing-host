use audiopus::{Application, Bitrate, Channels, SampleRate, coder::Encoder};

use crate::{AppResult, BUFFER_SIZE, error::AppError::XCustomMessage};

pub struct OpusEncoder {
    pub encoder: Encoder,
    accumulator: Vec<f32>,
    frame_size: usize,
    buffer: Vec<u8>,
}

impl OpusEncoder {
    pub fn new(channels: u16, sample_rate: u32) -> AppResult<Self> {
        let channels = match channels {
            1 => Channels::Mono,
            2 => Channels::Stereo,
            _ => return Err(XCustomMessage("Invalid channel format")),
        };
        let sample_rate = match sample_rate {
            8_000 => SampleRate::Hz8000,
            12_000 => SampleRate::Hz12000,
            16_000 => SampleRate::Hz16000,
            24_000 => SampleRate::Hz24000,
            48_000 => SampleRate::Hz48000,
            _ => return Err(XCustomMessage("Invalid sample rate")),
        };
        let mut encoder = Encoder::new(sample_rate, channels, Application::Audio)?;
        encoder.set_bitrate(Bitrate::Auto)?;
        Ok(Self {
            encoder,
            accumulator: Vec::new(),
            frame_size: BUFFER_SIZE * channels as usize,
            buffer: vec![0u8; 4000],
        })
    }

    pub fn encode(&mut self, pcm: &[f32]) -> AppResult<Vec<Vec<u8>>> {
        self.accumulator.extend_from_slice(pcm);

        let mut encoded_frames = Vec::new();

        println!(
            "accumulator: {} frame_size: {}",
            self.accumulator.len(),
            self.frame_size
        );
        while self.accumulator.len() > self.frame_size {
            let bytes_written = self
                .encoder
                .encode_float(&self.accumulator[..self.frame_size], &mut self.buffer)?;

            self.accumulator.drain(..self.frame_size);

            if bytes_written > 0 {
                encoded_frames.push(self.buffer[..bytes_written].to_vec());
            }
        }

        Ok(encoded_frames)
    }
}
