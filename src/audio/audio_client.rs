use crate::{AppResult, error::AppError::XCustomMessage};
use cpal::{
    BufferSize, Device, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use wasapi::{AudioCaptureClient, DeviceEnumerator, Handle, StreamMode, WaveFormat};
use windows::Win32::System::Threading::INFINITE;

pub struct CpalClient {
    device: Device,
    config: StreamConfig,
}

impl CpalClient {
    pub fn new() -> AppResult<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(XCustomMessage("No output device found"))?;
        println!("device: {}", device.description()?);
        let config = device.default_output_config()?;
        println!(
            "channels: {}, sample_rate: {}",
            config.channels(),
            config.sample_rate()
        );
        Ok(Self {
            device,
            config: config.config(),
        })
    }
    pub fn stream(&self, tx: tokio::sync::mpsc::Sender<Vec<f32>>) -> AppResult<Stream> {
        let tx = tx.clone();
        let stream = self.device.build_input_stream(
            StreamConfig {
                channels: self.config.channels,
                sample_rate: self.config.sample_rate,
                buffer_size: BufferSize::Fixed(960),
            },
            move |data: &[f32], _info| {
                let samples = data.to_vec();
                if let Err(e) = tx.try_send(samples) {
                    eprintln!("{}", e);
                }
            },
            |err| {
                eprintln!("{}", err);
            },
            None,
        )?;
        stream.play()?;
        Ok(stream)
    }
    pub fn get_channels(&self) -> u16 {
        self.config.channels
    }
    pub fn get_sample_rate(&self) -> u32 {
        self.config.sample_rate
    }
}

// pub struct AudioClient {
//     client: wasapi::AudioClient,
//     wave_format: WaveFormat,
//     audio_captureclient: AudioCaptureClient,
//     handle: Handle,
// }

// impl AudioClient {
//     pub fn new() -> AppResult<Self> {
//         wasapi::initialize_mta().ok()?;
//         let enumerator = DeviceEnumerator::new()?;
//         let device = enumerator.get_default_device(&wasapi::Direction::Render)?;
//         let mut audio_client = device.get_iaudioclient()?;
//         let desired_format = audio_client.get_mixformat()?;
//         let (_, min_time) = audio_client.get_device_period()?;
//         println!("min time: {}", min_time);
//         let mode = StreamMode::EventsShared {
//             autoconvert: false,
//             buffer_duration_hns: min_time,
//         };
//         audio_client.initialize_client(&desired_format, &wasapi::Direction::Capture, &mode)?;

//         println!("Initialized client");
//         let audio_captureclient = audio_client.get_audiocaptureclient()?;
//         let handle = audio_client.set_get_eventhandle()?;
//         Ok(Self {
//             client: audio_client,
//             wave_format: desired_format,
//             audio_captureclient,
//             handle,
//         })
//     }

//     pub fn start_stream(&self) -> AppResult<()> {
//         println!("audio stream started");
//         self.client.start_stream()?;
//         Ok(())
//     }
//     pub fn stop_client(&mut self) -> AppResult<()> {
//         self.client.stop_stream()?;
//         self.client.reset_stream()?;
//         Ok(())
//     }
//     pub fn capture_stream(&self) -> AppResult<Vec<f32>> {
//         let mut all_samples = Vec::new();
//         loop {
//             let packet_size = self.audio_captureclient.get_next_packet_size()?;
//             match packet_size {
//                 None | Some(0) => {
//                     break;
//                 }
//                 Some(size) => {
//                     let byte_count = size as usize
//                         * self.wave_format.get_nchannels() as usize
//                         * (self.wave_format.get_bitspersample() as usize / 8);
//                     let mut buffer = vec![0u8; byte_count];
//                     let (frames_read, buff_info) =
//                         self.audio_captureclient.read_from_device(&mut buffer)?;
//                     println!("{} and {}", size, frames_read);
//                     if frames_read == 0 {
//                         break;
//                     }
//                     if buff_info.flags.silent {
//                         let silence_count =
//                             frames_read as usize * self.wave_format.get_nchannels() as usize;
//                         all_samples.extend(vec![0.0f32; silence_count]);
//                     } else {
//                         match self.wave_format.get_bitspersample() {
//                             32 => {
//                                 let samples = bytemuck::cast_slice::<u8, f32>(
//                                     &buffer[..frames_read as usize
//                                         * self.wave_format.get_nchannels() as usize
//                                         * 4],
//                                 );
//                                 all_samples.extend(samples);
//                             }
//                             16 => {
//                                 let samples = bytemuck::cast_slice::<u8, i16>(
//                                     &buffer[..frames_read as usize
//                                         * self.wave_format.get_nchannels() as usize
//                                         * 2],
//                                 );
//                                 let sample_f32: Vec<f32> = samples
//                                     .iter()
//                                     .map(|&s| s as f32 / i16::MAX as f32)
//                                     .collect();
//                                 all_samples.extend(sample_f32);
//                             }
//                             _ => {
//                                 return Err(XCustomMessage("Unsupported bits format"));
//                             }
//                         }
//                     }
//                 }
//             }
//         }
//         Ok(all_samples)
//     }

//     pub fn capture_event_stream(&self) -> AppResult<Vec<f32>> {
//         match self.handle.wait_for_event(INFINITE) {
//             Ok(_) => return Ok(self.capture_stream()?),
//             Err(_) => return Ok(Vec::new()), // timeout = silence, not an error
//         };
//     }

//     pub fn get_waveformat(&self) -> &WaveFormat {
//         &self.wave_format
//     }
// }

// unsafe impl Send for AudioClient {}
