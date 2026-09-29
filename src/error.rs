use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("MDNS error: {0}")]
    MMdnsError(#[from] mdns_sd::Error),

    #[error("{0}")]
    XCustomMessage(&'static str),

    #[error("Tokio task join error: {0}")]
    TokioTaskError(#[from] tokio::task::JoinError),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Windows capture error: {0}")]
    CapturerError(#[from] windows::core::Error),

    #[error("RC gen error: {0}")]
    RcGenError(#[from] rcgen::Error),

    #[error("Rustls error: {0}")]
    RustlsError(#[from] quinn::rustls::Error),

    #[error("Address parse error: {0}")]
    AddrParseError(#[from] std::net::AddrParseError),

    #[error("Connection error: {0}")]
    ConnectionError(#[from] quinn::ConnectionError),

    #[error("Invalid InvalidLength: {0}")]
    InvalidLength(#[from] hmac::digest::InvalidLength),

    #[error("Write error: {0}")]
    WriteError(#[from] quinn::WriteError),

    #[error("Read exact error: {0}")]
    ReadExactError(#[from] quinn::ReadExactError),

    #[error("Wincode write error: {0}")]
    WinCodeWriteError(#[from] wincode::WriteError),

    #[error("Wincode read error: {0}")]
    WincodeReadError(#[from] wincode::ReadError),

    #[error("Closed Stream: {0}")]
    ClosedError(#[from] quinn::ClosedStream),

    #[error("Yuv error: {0}")]
    YuvError(#[from] yuv::YuvError),

    #[error("H264 error: {0}")]
    H264Error(#[from] openh264::Error),

    #[error("Send Datagram error: {0}")]
    SendDatagramError(#[from] quinn::SendDatagramError),

    #[error("Wasapi error: {0}")]
    WasApiError(#[from] wasapi::WasapiError),

    #[error("Opus error: {0}")]
    OpusError(#[from] audiopus::Error),

    #[error("cpal error: {0}")]
    CpalError(#[from] cpal::Error),
}
