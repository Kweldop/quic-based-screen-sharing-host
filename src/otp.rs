use hmac::{Hmac, KeyInit, Mac};
use quinn::Connection;
use sha2::Sha256;
use std::time::Duration;
use wincode::{SchemaRead, SchemaWrite};

use crate::{AppResult, error::AppError::XCustomMessage, server::Server};

type HmacSHA256 = Hmac<Sha256>;

#[derive(SchemaWrite, SchemaRead)]
pub enum HostMessage {
    OtpChallenge {
        nonce: [u8; 32],
    },
    SessionApproved {
        width: u32,
        height: u32,
        sample_rate: u32,
        channels: u8,
    },
    SessionDenied {
        reason: String,
    },
}

#[derive(SchemaWrite, SchemaRead)]
pub enum ClientMessage {
    OtpResponse { hmac: [u8; 32] },
}

pub struct OtpChallenge {
    otp: String,
    nonce: [u8; 32],
}

impl OtpChallenge {
    pub fn new() -> Self {
        Self {
            otp: Self::generate_otp(),
            nonce: Self::generate_nonce(),
        }
    }

    fn generate_otp() -> String {
        let otp = rand::random_range(0..10000u32);
        format!("{:04}", otp) // always 4 digits with leading zeros
    }
    fn generate_nonce() -> [u8; 32] {
        rand::random()
    }

    pub fn verify_hmac(&self, recieved_hmac: &[u8; 32]) -> AppResult<bool> {
        let mut mac = HmacSHA256::new_from_slice(self.otp.as_bytes())?;
        mac.update(&self.nonce);
        let verify = mac.verify(recieved_hmac.into()).is_ok();
        Ok(verify)
    }

    pub async fn handle_otp_connection(
        &self,
        connection: &Connection,
        width: u32,
        height: u32,
        sample_rate: u32,
        channels: u8,
    ) -> AppResult<()> {
        let (mut send, mut recv) = connection.open_bi().await?;

        println!("OTP: {}", self.otp);
        Server::send_message(&mut send, &HostMessage::OtpChallenge { nonce: self.nonce }).await?;
        println!("Otp challenge sent!");
        let response = tokio::select! {
            msg= Server::read_message::<ClientMessage>(&mut recv)=>msg?,
            _=tokio::time::sleep(Duration::from_secs(60))=>{
                Server::send_message(&mut send, &HostMessage::SessionDenied { reason: "otp_expired".into() }).await?;
                return Err(XCustomMessage("OTP Expired"));
            },
        };
        match response {
            ClientMessage::OtpResponse { hmac } => {
                if self.verify_hmac(&hmac)? {
                    println!("OTP verified!");
                    Server::send_message(
                        &mut send,
                        &HostMessage::SessionApproved {
                            width: width,
                            height: height,
                            sample_rate: sample_rate,
                            channels: channels,
                        },
                    )
                    .await?;
                    send.finish()?;
                } else {
                    println!("OTP wrong - session denied!");
                    Server::send_message(
                        &mut send,
                        &HostMessage::SessionDenied {
                            reason: "Incorrect otp".into(),
                        },
                    )
                    .await?;
                    send.finish()?;
                    return Err(XCustomMessage("Incorrect OTP"));
                }
            }
        }
        Ok(())
    }
}
