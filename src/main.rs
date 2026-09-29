use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use quinn::Connection;
use wincode::{SchemaRead, SchemaWrite};

use crate::{
    audio::{AudioChunk, audio_client::CpalClient, opus_encoder::OpusEncoder},
    error::AppError::{self},
    mdns::Mdns,
    otp::OtpChallenge,
    server::Server,
    video::{
        VideoChunk,
        display_capturer::{DxgiCapture, RawFrame},
        encoder::VideoEncoder,
    },
};

pub type AppResult<T> = Result<T, AppError>;

mod audio;
mod error;
mod mdns;
mod otp;
mod server;
mod video;

#[cfg(test)]
mod test;

#[derive(Debug, SchemaRead, SchemaWrite)]
pub enum MediaPacket {
    VideoChunk(VideoChunk),
    AudioChunk(AudioChunk),
}

const BUFFER_SIZE: usize = 480;

#[tokio::main]
async fn main() -> AppResult<()> {
    // --- setup ---
    let server = Arc::new(Server::new()?);
    let mdns = Arc::new(Mutex::new(Mdns::new()?));

    // add cert fingerprint to mDNS properties
    mdns.lock()
        .unwrap()
        .add_prop("encoded_fp".into(), server.get_encoded_cert());
    // --- start mDNS on blocking thread ---
    let monitor_rx = mdns.lock().unwrap().start_service()?;
    tokio::task::spawn_blocking(async move || -> AppResult<()> {
        Mdns::monitor_daemon(monitor_rx)?;
        Ok(())
    });

    println!("mDNS service started successfully.");

    // --- start QUIC accept loop ---
    let server_accept = Arc::clone(&server);
    let tokio_handle = tokio::spawn(async move {
        match server_accept.endpoint.accept().await {
            Some(incoming) => {
                println!("connecting...");
                //Connect client
                let connection = match incoming.await {
                    Ok(conn) => {
                        println!("connected to {}", conn.remote_address());
                        Arc::new(conn)
                    }
                    Err(e) => {
                        eprintln!("connection error: {:?}", e);
                        return;
                    }
                };

                //Get DxgiCapture client
                let capture = match DxgiCapture::new() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("{}", e);
                        return;
                    }
                };
                let (width, height) = capture.get_wh();

                //Get audio client
                // let audio_client = match AudioClient::new() {
                //     Ok(a) => a,
                //     Err(e) => {
                //         eprintln!("{}", e);
                //         return;
                //     }
                // };

                let audio_client = match CpalClient::new() {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("{}", e);
                        return;
                    }
                };
                let sample_rate = audio_client.get_sample_rate();
                let channels = audio_client.get_channels() as u8;

                //Get OTP challenge
                let otp_challenge = OtpChallenge::new();
                if let Err(e) = otp_challenge
                    .handle_otp_connection(&connection, width, height, sample_rate, channels)
                    .await
                {
                    eprintln!("{}", e);
                    return;
                };

                //Video stream
                let video_connection = Arc::clone(&connection);
                let video_handle = tokio::spawn(async move {
                    if let Err(e) = start_video_stream(video_connection, capture).await {
                        eprintln!("{}", e);
                        return;
                    }
                });

                //Audio stream
                let audio_connection = Arc::clone(&connection);
                let audio_handle = tokio::spawn(async move {
                    if let Err(e) = audio_cpal_stream(audio_connection, audio_client).await {
                        eprintln!("{}", e);
                        return;
                    };
                });

                //let recv_handle = tokio::spawn(async move {});

                tokio::select! {
                    _ = video_handle => {},
                    _=  audio_handle=>{},
                  //  _ = recv_handle => {}
                };

                connection.closed().await;
                println!("client disconnected");
            }
            None => {
                println!("endpoint closed");
                return;
            }
        }
    });
    tokio_handle.await?;
    // --- wait for shutdown ---
    // tokio::signal::ctrl_c().await?;
    println!("shutting down...");

    // --- clean shutdown ---
    server.endpoint.close(0u32.into(), b"server shutdown");
    mdns.lock().unwrap().stop_service()?;

    Ok(())
}

async fn start_video_stream(connection: Arc<Connection>, capture: DxgiCapture) -> AppResult<()> {
    // let t0 = std::time::Instant::now();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<RawFrame>(1);

    let (width, height) = capture.get_wh();
    tokio::spawn(async move {
        let mut capture = capture;
        loop {
            let frame = match capture.capture_frame() {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("{}", e);
                    return;
                }
            };
            match frame {
                Some(frame) => {
                    // println!("captured: {}ms", t0.elapsed().as_millis());
                    if let Err(e) = tx.send(frame).await {
                        eprintln!("{}", e);
                        break;
                    };
                }
                None => {
                    //   println!("No frame captured");
                }
            }
        }
    });

    let mut encoder = VideoEncoder::new(width, height)?;

    while let Some(frame) = rx.recv().await {
        let yuv = frame.to_yuv420()?;
        //  println!("before encode: {}ms", t0.elapsed().as_millis());
        let nal_bytes = match encoder.encode(&yuv) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{}", e);
                continue;
            }
        };
        // println!("encode: {}ms", t0.elapsed().as_millis());

        if nal_bytes.is_empty() {
            continue;
        };
        let frame_id = encoder.frame_id;
        let is_keyframe = frame_id % 60 == 1;
        let chunks = VideoChunk::chunk_frame(frame_id, &nal_bytes, is_keyframe);
        let _ = chunks
            .iter()
            .map(|chunk| {
                let bytes =
                    wincode::serialize::<MediaPacket>(&MediaPacket::VideoChunk(chunk.clone()))
                        .unwrap();
                if let Err(e) = connection.send_datagram(bytes.into()) {
                    eprintln!("{}", e);
                    return;
                };
            })
            .collect::<Vec<_>>();
    }

    Ok(())
}

async fn audio_cpal_stream(connection: Arc<Connection>, client: CpalClient) -> AppResult<()> {
    let (stream_tx, mut stream_rx) = tokio::sync::mpsc::channel(32);
    let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
    let _stream = client.stream(stream_tx)?;
    let t0 = Instant::now();

    tokio::spawn(async move {
        let mut encoder = match OpusEncoder::new(client.get_channels(), client.get_sample_rate()) {
            Ok(en) => en,
            Err(e) => {
                eprintln!("{}", e);
                return;
            }
        };
        while let Some(data) = stream_rx.recv().await {
            let encoded = encoder.encode(&data);
            match encoded {
                Ok(frames) => {
                    for e in frames {
                        if let Err(e) = audio_tx.send(e.to_vec()).await {
                            eprintln!("{}", e);
                            return;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{}", e);
                    return;
                }
            }
        }
    });
    let mut sequence = 0u32;
    while let Some(data) = audio_rx.recv().await {
        let audio_chunk = AudioChunk {
            sequence,
            payload: data,
        };
        let serialized = wincode::serialize(&MediaPacket::AudioChunk(audio_chunk))?;

        connection.send_datagram(serialized.into())?;
        println!(
            "sent sqeuence {} audio at {}",
            sequence,
            t0.elapsed().as_millis()
        );
        sequence += 1;
    }
    Ok(())
}

// async fn start_audio_stream(connection: Arc<Connection>, mut client: AudioClient) -> AppResult<()> {
//     let t0 = Instant::now();
//     client.start_stream()?;
//     let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(4);

//     tokio::spawn(async move {
//         let wavformat = client.get_waveformat();
//         let mut encoder =
//             match OpusEncoder::new(wavformat.get_nchannels(), wavformat.get_samplespersec()) {
//                 Ok(en) => en,
//                 Err(e) => {
//                     eprintln!("{}", e);
//                     let _ = client.stop_client();
//                     return;
//                 }
//             };
//         while let Ok(data) = client.capture_event_stream() {
//             // println!("recieved audio at {}", t0.elapsed().as_millis());
//             let encoded_data = encoder.encode(&data);

//             match encoded_data {
//                 Ok(frames) => {
//                     for e in frames {
//                         // println!("encoded audio at {}", t0.elapsed().as_millis());
//                         if let Err(e) = audio_tx.send(e.to_vec()).await {
//                             eprintln!("{}", e);
//                             return;
//                         }
//                     }
//                 }
//                 Err(e) => {
//                     eprintln!("{}", e);
//                     let _ = client.stop_client();
//                     return;
//                 }
//             }
//         }
//     });
//     let mut sequence = 0u32;
//     while let Some(data) = audio_rx.recv().await {
//         let audio_chunk = AudioChunk {
//             sequence,
//             payload: data,
//         };
//         let serialized = wincode::serialize(&MediaPacket::AudioChunk(audio_chunk))?;

//         connection.send_datagram(serialized.into())?;
//         println!(
//             "sent sqeuence {} audio at {}",
//             sequence,
//             t0.elapsed().as_millis()
//         );
//         sequence += 1;
//     }
//     Ok(())
// }
