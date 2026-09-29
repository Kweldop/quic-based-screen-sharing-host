use crate::video::VideoChunk;

const MAX_CHUNK_SIZE: u32 = 1100;

impl VideoChunk {
    pub fn chunk_frame(
        frame_id: u32,
        nal_bytes: &[u8],
        // width: u32,
        // height: u32,
        is_keyframe: bool,
    ) -> Vec<Self> {
        let chunks: Vec<&[u8]> = nal_bytes.chunks(MAX_CHUNK_SIZE as usize).collect();
        let total = chunks.len() as u16;
        chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| VideoChunk {
                frame_id,
                chunk_index: i as u16,
                total_chunks: total,
                // width,
                // height,
                is_keyframe,
                payload: chunk.to_vec(),
            })
            .collect()
    }
}
