
use ank_proto::v1::siren::SirenEvent;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

pub struct SentenceAccumulator {
    buffer: String,
    tts_tx: mpsc::Sender<String>,
}

impl SentenceAccumulator {
    pub fn new(tts_tx: mpsc::Sender<String>) -> Self {
        Self {
            buffer: String::with_capacity(256),
            tts_tx,
        }
    }

    pub async fn push_token(&mut self, token: &str) {
        self.buffer.push_str(token);

        if token.contains('.') || token.contains('?') || token.contains('!') || token.contains('\n') {
            let sentence = self.buffer.trim().to_string();
            if !sentence.is_empty() {
                if let Err(e) = self.tts_tx.send(sentence.clone()).await {
                    warn!("TTS Worker disconnected: {}", e);
                } else {
                    info!("Sentence accumulated for TTS: '{}'", sentence);
                }
            }
            self.buffer.clear();
        }
    }

    pub async fn flush(&mut self) {
        let sentence = self.buffer.trim().to_string();
        if !sentence.is_empty() {
            let _ = self.tts_tx.send(sentence).await;
        }
        self.buffer.clear();
    }
}

pub fn spawn_tts_worker(
    mut rx: mpsc::Receiver<String>,
    siren_tx: mpsc::Sender<Result<SirenEvent, tonic::Status>>,
    sequence_number_start: u64,
) {
    tokio::task::spawn_blocking(move || {
        let mut seq = sequence_number_start;

        while let Some(sentence) = rx.blocking_recv() {
            info!("TTS Worker synthesizing: '{}'", sentence);

            // Mock TTS: Generar 1/4 segundo de PCM (22050Hz, 16-bit)
            let audio_len = 22050 * 2 / 4;
            let mut mock_audio = vec![0u8; audio_len];

            for i in 0..audio_len {
                mock_audio[i] = (i % 256) as u8;
            }

            seq += 1;
            let event = SirenEvent {
                event_type: "TTS_AUDIO".to_string(),
                message: sentence,
                processed_sequence_number: seq,
                tts_audio_chunk: mock_audio,
                sample_rate: 22050,
            };

            if siren_tx.blocking_send(Ok(event)).is_err() {
                warn!("Siren Stream disconnected. Terminating TTS worker.");
                break;
            }
        }
    });
}
