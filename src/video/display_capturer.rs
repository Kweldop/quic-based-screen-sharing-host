use core::slice;

use windows::{
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            Direct3D11::{
                D3D11_CPU_ACCESS_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
                D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11DeviceContext, ID3D11Texture2D,
            },
            Dxgi::{
                Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
                DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
                IDXGIDevice, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource,
            },
        },
    },
    core::Interface,
};

use crate::error::AppError::XCustomMessage;
use crate::{AppResult, error};

pub struct RawFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}

#[derive(Debug)]
pub struct DxgiCapture {
    context: ID3D11DeviceContext, // GPU command context for issuing draw/copy commands
    duplication: IDXGIOutputDuplication, // Desktop duplication object that captures screen
    staging: ID3D11Texture2D,     // Staging texture (CPU-accessible copy of screen)
    width: u32,                   // Screen width in pixels
    height: u32,                  // Screen height in pixels
}

impl DxgiCapture {
    // Initializes the DXGI capture pipeline - sets up GPU device, display output, and staging texture
    pub fn new() -> AppResult<Self> {
        unsafe {
            // Step 1: Create Direct3D 11 Device and Context
            // These are needed to communicate with the GPU and issue rendering/copy commands
            let mut device = None;
            let mut context = None;
            D3D11CreateDevice(
                None,                                                              // Use default adapter
                D3D_DRIVER_TYPE_HARDWARE, // Use GPU (hardware acceleration)
                HMODULE::default(),       // No custom driver
                windows::Win32::Graphics::Direct3D11::D3D11_CREATE_DEVICE_FLAG(0), // No special device flags
                None,               // Use default feature levels
                D3D11_SDK_VERSION,  // SDK version
                Some(&mut device),  // Output: GPU device
                None,               // Output: feature level (unused)
                Some(&mut context), // Output: GPU context
            )?;

            // Step 2: Extract and validate the device and context
            let device = device.ok_or(XCustomMessage("Couldn't find device"))?;
            let context = context.ok_or(XCustomMessage("Couldn't find context"))?;

            // Step 3: Get the DXGI Device interface
            // DXGI is the layer on top of Direct3D that handles display/window operations
            let dxgi_device: IDXGIDevice = device.cast()?;

            // Step 4: Get the adapter (graphics card) from the device
            let adapter = dxgi_device.GetAdapter()?;

            // Step 5: Get the first output (primary monitor, index 0)
            // Index 0 = primary display, 1 = secondary, etc.
            let output = adapter.EnumOutputs(0)?;

            // Step 6: Cast to IDXGIOutput1 to access desktop duplication features
            // Desktop duplication requires the newer Output1 interface
            let output1: IDXGIOutput1 = output.cast()?;

            // Step 7: Enable desktop duplication on this output
            // This allows us to capture the screen content as frames
            let duplication = output1.DuplicateOutput(&device)?;

            // Step 8: Get the description of the duplicated output (screen dimensions, etc.)
            let desc = duplication.GetDesc();

            // Step 9: Create a staging texture descriptor
            // A staging texture is a CPU-accessible copy of GPU memory
            // We'll copy GPU screen data here so the CPU can read it
            let staging_desc = D3D11_TEXTURE2D_DESC {
                Width: desc.ModeDesc.Width,         // Match screen width
                Height: desc.ModeDesc.Height,       // Match screen height
                MipLevels: 1,                       // No mipmaps (single resolution)
                ArraySize: 1,                       // Single texture (not an array)
                Format: DXGI_FORMAT_B8G8R8A8_UNORM, // Color format: BGRA 8-bit per channel
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,   // No anti-aliasing
                    Quality: 0, // No quality options
                },
                Usage: D3D11_USAGE_STAGING, // Staging mode: GPU->CPU transfer
                BindFlags: 0,               // Not bound to any pipeline stage
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32, // CPU can read this texture
                MiscFlags: Default::default(), // No special flags
            };

            // Step 10: Create the actual staging texture on the GPU
            let mut staging: Option<ID3D11Texture2D> = None;
            device.CreateTexture2D(&staging_desc, None, Some(&mut staging))?;
            // Step 11: Return the initialized DxgiCapture structure
            return Ok(Self {
                context: context,
                duplication: duplication,
                staging: staging.ok_or(XCustomMessage("No staging found"))?,
                width: desc.ModeDesc.Width,
                height: desc.ModeDesc.Height,
            });
        };
    }

    // Captures a single frame from the screen
    // Returns Ok(Some(frame)) if a new frame is available
    // Returns Ok(None) if no frame is available (timeout)
    // Returns Err if an error occurs
    pub fn capture_frame(&mut self) -> AppResult<Option<RawFrame>> {
        unsafe {
            // Step 1: Initialize frame metadata and resource containers
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default(); // Will hold frame metadata
            let mut resource: Option<IDXGIResource> = None; // Will hold the GPU texture

            // Step 2: Try to acquire the next available frame from the duplicated output
            // 8 = timeout in milliseconds (returns None if no frame in 8ms)
            let result = self
                .duplication
                .AcquireNextFrame(8, &mut frame_info, &mut resource);

            // Step 3: Handle the result of AcquireNextFrame
            match result {
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                    // No new frame available within timeout period
                    // This is normal and expected when the screen hasn't changed
                    // println!("DXGI_ERROR_WAIT_TIMEOUT");
                    return Ok(None);
                }
                Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => {
                    // Access lost (screen locked by another app, fullscreen game, etc.)
                    // Caller needs to recreate the DxgiCapture to recover
                    eprintln!("DXGI access lost — recreate duplication");
                    return Err(error::AppError::CapturerError(e));
                }
                Err(e) => {
                    // Some other error occurred
                    eprintln!("AcquireNextFrame error: {:?}", e);
                    return Err(error::AppError::CapturerError(e));
                }
                _ => {
                    // Frame acquired successfully, continue processing
                }
            }

            // Step 4: Extract the GPU texture from the result
            // This texture contains the current screen frame on the GPU
            let resource = resource.ok_or(XCustomMessage("No duplicated resource"))?;

            // Step 5: Cast the generic resource to a 2D texture
            let gpu_tex: ID3D11Texture2D = resource.cast()?;

            // Step 6: Copy the GPU texture to the staging texture
            // Staging texture is CPU-accessible, GPU texture is not
            // After this, staging texture contains the screen data
            self.context.CopyResource(&self.staging, &gpu_tex);

            // Step 7: Create a mapped subresource (pointer to texture data + metadata)
            let mut mapped = Default::default();

            // Step 8: Map the staging texture for CPU reading
            // This allows us to access the texture data as a pointer
            self.context.Map(
                &self.staging,                                        // Texture to map
                0, // Subresource index (0 = first/only)
                windows::Win32::Graphics::Direct3D11::D3D11_MAP_READ, // Read-only access
                0, // No special flags
                Some(&mut mapped), // Output: mapped data info
            )?;

            // Step 9: Calculate the total byte count of the frame
            // stride = bytes per row, height = number of rows
            let stride = mapped.RowPitch;
            let byte_count = (stride * self.height) as usize;

            // Step 10: Create a safe slice from the raw pointer and copy to vector
            // mapped.pData points to the beginning of the texture data in memory
            // We convert it to a slice and then to a Vec to own the data
            let data = slice::from_raw_parts(mapped.pData as *const u8, byte_count).to_vec();

            // Step 11: Unmap the staging texture (release access)
            // This tells GPU we're done reading and it can use the texture again
            self.context.Unmap(&self.staging, 0);

            // Step 12: Release the current frame back to the duplication
            // This tells DXGI we're done with this frame and ready for the next one
            self.duplication.ReleaseFrame()?;

            // Step 13: Return the captured frame data wrapped in RawFrame
            return Ok(Some(RawFrame {
                data, // Pixel data (BGRA format)
                width: self.width,
                height: self.height,
                stride, // Bytes per row
            }));
        }
    }

    // Simple getter to retrieve the screen dimensions
    // Returns a tuple of (width, height) in pixels
    pub fn get_wh(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
