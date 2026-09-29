use cpal::traits::StreamTrait;

use crate::{AppResult, audio::audio_client::CpalClient};

#[tokio::test]
async fn testcpal() -> AppResult<()> {
    let cpalclient = CpalClient::new()?;
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    println!("Cpal client worked!");
    let _stream = cpalclient.stream(tx)?;

    while let Some(data) = rx.recv().await {
        println!("{}", data.len());
    }

    Ok(())
}
