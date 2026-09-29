use yuv::{
    YuvConversionMode, YuvPlanarImageMut, YuvRange, YuvStandardMatrix,
    bgra_to_yuv420 as crate_bgra_to_yuv420,
};

use crate::{AppResult, video::display_capturer::RawFrame};

impl RawFrame {
    pub fn to_yuv420(&self) -> AppResult<Vec<u8>> {
        let w = self.width as usize;
        let h = self.height as usize;
        let _s = self.stride as usize;

        let y_size = w * h;
        let uv_width = w / 2;
        let uv_size = uv_width * (h / 2);

        let mut yuv = vec![0u8; y_size + uv_size * 2];

        let (y_plane, uv_planes) = yuv.split_at_mut(y_size);
        let (u_plane, v_plane) = uv_planes.split_at_mut(uv_size);

        let mut planar_image = YuvPlanarImageMut {
            y_plane: yuv::BufferStoreMut::Borrowed(y_plane),
            y_stride: self.width,
            u_plane: yuv::BufferStoreMut::Borrowed(u_plane),
            u_stride: uv_width as u32,
            v_plane: yuv::BufferStoreMut::Borrowed(v_plane),
            v_stride: uv_width as u32,
            width: self.width,
            height: self.height,
        };

        crate_bgra_to_yuv420(
            &mut planar_image,
            &self.data,
            self.stride,
            YuvRange::Full,
            YuvStandardMatrix::Bt601,
            YuvConversionMode::Balanced,
        )?;

        Ok(yuv)
    }
}
