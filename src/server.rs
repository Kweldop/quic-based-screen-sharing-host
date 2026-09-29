use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use crate::{AppResult, error::AppError::XCustomMessage};
use quinn::{
    Endpoint, RecvStream, SendStream, ServerConfig,
    rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer},
};

use sha2::Digest;
use wincode::{SchemaRead, SchemaWrite, config::Configuration, len::UseIntLen};

pub struct Server {
    pub endpoint: Endpoint,
    cert: CertificateDer<'static>,
}

impl Server {
    pub fn new() -> AppResult<Self> {
        let (server_config, cert) = Self::configure_server()?;
        let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 1700);
        let endpoint = Endpoint::server(server_config, server_addr)?;
        Ok(Self { endpoint, cert })
    }

    pub fn configure_server() -> AppResult<(ServerConfig, CertificateDer<'static>)> {
        let dir = directories::ProjectDirs::from("com", "Remote Desktop", "Remote Desktop Host")
            .ok_or(XCustomMessage("No directpry found"))?;
        let path = dir.data_local_dir();
        std::fs::create_dir_all(path)?;
        let cert_path = path.join("cert.der");
        let key_path = path.join("key.der");
        let cert_der: CertificateDer<'_>;
        let mut server_config: ServerConfig;
        if cert_path.exists() && key_path.exists() {
            cert_der = CertificateDer::from(std::fs::read(cert_path)?);
            let key_der = PrivatePkcs8KeyDer::from(std::fs::read(key_path)?);
            server_config = ServerConfig::with_single_cert(vec![cert_der.clone()], key_der.into())?;
        } else {
            let hostname = sysinfo::System::host_name().ok_or(XCustomMessage("No hostname"))?;
            println!("{hostname}");
            let cert = rcgen::generate_simple_self_signed(vec![hostname, "localhost".into()])?;
            cert_der = CertificateDer::from(cert.cert);
            let key_bytes = cert.signing_key.serialize_der();
            // after generating cert and key
            std::fs::write(&cert_path, &cert_der)?;
            std::fs::write(&key_path, &key_bytes)?;
            let priv_key = PrivatePkcs8KeyDer::from(key_bytes);
            server_config =
                ServerConfig::with_single_cert(vec![cert_der.clone()], priv_key.into())?;
        }
        Self::apply_transport_config(&mut server_config);
        Ok((server_config, cert_der))
    }

    fn apply_transport_config(server_config: &mut ServerConfig) {
        let transport = Arc::get_mut(&mut server_config.transport).unwrap();
        transport.max_concurrent_uni_streams(0_u8.into());
    }

    pub fn get_encoded_cert(&self) -> String {
        hex::encode(sha2::Sha256::digest(self.cert.as_ref()))
    }

    pub async fn send_message<T>(send: &mut SendStream, message: &T) -> AppResult<()>
    where
        T: SchemaWrite<Configuration<true, 4194304, UseIntLen<u64, 0>>, Src = T>,
    {
        let serialized_message = wincode::serialize::<T>(message)?;
        let len = serialized_message.len() as u32;
        send.write_all(&len.to_be_bytes()).await?;
        send.write_all(&serialized_message).await?;
        Ok(())
    }
    pub async fn read_message<T>(recv: &mut RecvStream) -> AppResult<T>
    where
        for<'a> T: SchemaRead<'a, Configuration<true, 4194304, UseIntLen<u64, 0>>, Dst = T>,
    {
        let mut len = [0u8; 4];
        recv.read_exact(&mut len).await?;
        let len = u32::from_be_bytes(len) as usize;
        let mut buf = vec![0u8; len];
        recv.read_exact(&mut buf).await?;
        Ok(wincode::deserialize::<T>(&buf)?)
    }
}
