use wincode::{SchemaRead, SchemaWrite};

pub mod audio_client;
pub mod opus_encoder;

#[derive(Debug, SchemaRead, SchemaWrite)]
pub struct AudioChunk {
    pub sequence: u32,
    pub payload: Vec<u8>,
}
