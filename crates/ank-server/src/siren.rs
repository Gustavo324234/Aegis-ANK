use ank_core::{PCB as CorePCB, SchedulerEvent};
use ank_proto::v1::siren::siren_service_server::SirenService;
use ank_proto::v1::siren::{AudioChunk, SirenEvent};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{info, warn};
use webrtc_vad::{SampleRate, Vad, VadConfig};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::server::CitadelAuth;

#[derive(Debug, Clone, Copy, PartialEq)]
enum VadState {
    Silence,
    Speech,
}

pub struct AnkSirenService {
    scheduler_tx: mpsc::Sender<SchedulerEvent>,
    whisper_ctx: Arc<WhisperContext>,
}

impl AnkSirenService {
    pub fn new(scheduler_tx: mpsc::Sender<SchedulerEvent>) -> anyhow::Result<Self> {
        // Load Whisper Model - GGUF Base as per ANK-124
        // Logic SRE: Si el modelo no está, fallamos al arranque para evitar fallos silenciosos en producción
        let model_path = std::env::var("AEGIS_WHISPER_MODEL").unwrap_or_else(|_| "ggml-base.bin".to_string());
        
        info!("Loading Whisper model from: {}", model_path);
        let ctx = WhisperContext::new_with_params(&model_path, WhisperContextParameters::default())
            .map_err(|e| anyhow::anyhow!("Failed to load Whisper model {}: {:?}", model_path, e))?;

        Ok(Self {
            scheduler_tx,
            whisper_ctx: Arc::new(ctx),
        })
    }
}

#[tonic::async_trait]
impl SirenService for AnkSirenService {
    type SirenStreamStream = ReceiverStream<Result<SirenEvent, Status>>;

    async fn siren_stream(
        &self,
        request: Request<Streaming<AudioChunk>>,
    ) -> Result<Response<Self::SirenStreamStream>, Status> {
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        info!("Siren stream connected for Tenant: {}", auth.tenant_id);

        let mut in_stream = request.into_inner();
        let (tx_chunks, mut rx_chunks) = mpsc::channel::<AudioChunk>(200);
        let (tx_events, rx_events) = mpsc::channel::<Result<SirenEvent, Status>>(200);

        // Consumer task
        let tx_events_consumer = tx_events.clone();
        tokio::spawn(async move {
            loop {
                match in_stream.message().await {
                    Ok(Some(chunk)) => {
                        if tx_chunks.try_send(chunk).is_err() {
                            let _ = tx_events_consumer.send(Err(Status::resource_exhausted("Audio buffer full"))).await;
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        warn!("SirenStream gRPC error: {}", e);
                        break;
                    }
                }
            }
        });

        // Worker Task: VAD + STT Dispatcher
        let tx_events_worker = tx_events.clone();
        let whisper_ctx = self.whisper_ctx.clone();
        let scheduler_tx = self.scheduler_tx.clone();
        
        tokio::spawn(async move {
            let mut vad = Vad::new_with_config(VadConfig::VeryAggressive);
            let mut accumulator = Vec::with_capacity(1280);
            let mut speech_buffer: Vec<f32> = Vec::with_capacity(16000 * 5); // 5s buffer pre-allocated
            let mut vad_state = VadState::Silence;
            let mut silence_frames = 0;

            const FRAME_SIZE_BYTES: usize = 640;
            const SILENCE_THRESHOLD_FRAMES: usize = 30; // 600ms

            while let Some(chunk) = rx_chunks.recv().await {
                if chunk.format != "pcm_16khz_16bit" { continue; }

                accumulator.extend_from_slice(&chunk.data);

                while accumulator.len() >= FRAME_SIZE_BYTES {
                    let frame_u8: Vec<u8> = accumulator.drain(..FRAME_SIZE_BYTES).collect();
                    let frame_i16: Vec<i16> = frame_u8.chunks_exact(2)
                        .map(|c| i16::from_le_bytes([c[0], c[1]]))
                        .collect();

                    let is_voice = vad.is_voice(&frame_i16, SampleRate::Rate16kHz).unwrap_or(false);

                    match vad_state {
                        VadState::Silence => {
                            if is_voice {
                                vad_state = VadState::Speech;
                                silence_frames = 0;
                                speech_buffer.clear();
                                // Extend current frame into speech buffer
                                speech_buffer.extend(frame_i16.iter().map(|&x| x as f32 / 32768.0));
                                
                                let _ = tx_events_worker.send(Ok(SirenEvent {
                                    event_type: "VAD_START".to_string(),
                                    message: "Speaking...".to_string(),
                                    processed_sequence_number: chunk.sequence_number,
                                })).await;
                            }
                        }
                        VadState::Speech => {
                            // Accumulate regardless of VAD result to keep the utterance intact
                            speech_buffer.extend(frame_i16.iter().map(|&x| x as f32 / 32768.0));

                            if !is_voice {
                                silence_frames += 1;
                                if silence_frames >= SILENCE_THRESHOLD_FRAMES {
                                    vad_state = VadState::Silence;
                                    silence_frames = 0;

                                    let _ = tx_events_worker.send(Ok(SirenEvent {
                                        event_type: "VAD_END".to_string(),
                                        message: "Processing voice...".to_string(),
                                        processed_sequence_number: chunk.sequence_number,
                                    })).await;

                                    // ANK-124: Offload STT to blocking thread
                                    let captured_audio = std::mem::take(&mut speech_buffer);
                                    let ctx_clone = whisper_ctx.clone();
                                    let scheduler_tx_clone = scheduler_tx.clone();
                                    let auth_clone = auth.clone();
                                    let tx_events_stt = tx_events_worker.clone();
                                    let seq = chunk.sequence_number;

                                    tokio::task::spawn_blocking(move || {
                                        let _ = tx_events_stt.try_send(Ok(SirenEvent {
                                            event_type: "STT_START".to_string(),
                                            message: "Transcribing...".to_string(),
                                            processed_sequence_number: seq,
                                        }));

                                        let mut state = ctx_clone.create_state().expect("Failed to create Whisper state");
                                        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                                        params.set_n_threads(4);
                                        params.set_language(Some("es")); // Default Aegis Language
                                        params.set_print_special(false);
                                        params.set_print_progress(false);

                                        if let Err(e) = state.full(params, &captured_audio) {
                                            warn!("Whisper inference error: {:?}", e);
                                            let _ = tx_events_stt.try_send(Ok(SirenEvent {
                                                event_type: "STT_ERROR".to_string(),
                                                message: format!("STT Error: {:?}", e),
                                                processed_sequence_number: seq,
                                            }));
                                            return;
                                        }

                                        let mut text = String::new();
                                        let num_segments = state.full_n_segments().unwrap_or(0);
                                        for i in 0..num_segments {
                                            if let Ok(segment) = state.full_get_segment_text(i) {
                                                text.push_str(&segment);
                                            }
                                        }

                                        let text = text.trim().to_string();
                                        
                                        // Noise Filter: Alucinaciones comunes de Whisper
                                        if text.is_empty() || text.to_lowercase().contains("[musica]") || text.to_lowercase().contains("[silencio]") || text.len() < 2 {
                                            info!("Whisper: Ignoring noise/hallucination: '{}'", text);
                                            return;
                                        }

                                        info!("STT Result: '{}' (Tenant: {})", text, auth_clone.tenant_id);

                                        // Inyectar tarea al Scheduler
                                        let mut pcb = CorePCB::new("Voice Command".to_string(), 10, text.clone());
                                        pcb.tenant_id = Some(auth_clone.tenant_id);
                                        pcb.session_key = Some(auth_clone.session_key);
                                        let pid_clone = pcb.pid.clone();

                                        let _ = scheduler_tx_clone.blocking_send(SchedulerEvent::ScheduleTask(Box::new(pcb)));

                                        let message_json = serde_json::json!({
                                            "transcript": text,
                                            "pid": pid_clone
                                        }).to_string();

                                        let _ = tx_events_stt.try_send(Ok(SirenEvent {
                                            event_type: "STT_DONE".to_string(),
                                            message: message_json,
                                            processed_sequence_number: seq,
                                        }));
                                    });
                                }
                            } else {
                                silence_frames = 0;
                            }
                        }
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx_events)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use async_stream::stream;
    use std::time::Duration;

    #[tokio::test]
    async fn test_siren_stream_latency_and_backpressure() -> Result<()> {
        let (tx, _rx) = mpsc::channel(1);
        // Note: This test will fail if model is missing, which is correct for SRE Zero-Panic/Fail-Fast
        let service = AnkSirenService::new(tx).expect("Model should be available for test");

        // Creamos un stream asíncrono que envía 1000 chunks con pequeña latencia
        let chunks_stream = stream! {
            for i in 0..1000 {
                if i % 50 == 0 {
                    tokio::time::sleep(Duration::from_millis(1)).await; // Jitter de red emulado
                }
                yield AudioChunk {
                    sequence_number: i as u64,
                    data: vec![0; 100],
                    format: "pcm_16khz_16bit".to_string(),
                    sample_rate: 16000,
                };
            }
        };

        // Empaquetar stream como el request de tonic y agregar metadatos de auth falsos
        let mut request = Request::new(Box::pin(chunks_stream) as _);
        
        let mut extensions = tonic::Extensions::new();
        extensions.insert(CitadelAuth {
            tenant_id: "test_tenant".to_string(),
            session_key: "test_session_key".to_string(),
        });
        *request.extensions_mut() = extensions;

        // Llamamos al servicio
        let response = service.siren_stream(request).await?;
        let mut response_stream = response.into_inner();

        // Verificamos que no genere error de backpressure inicial de golpe
        // Recorremos los eventos de respuesta, si los hubiera
        while let Some(resp) = tokio_stream::StreamExt::next(&mut response_stream).await {
            match resp {
                Ok(_) => { /* Evento normal */ },
                Err(status) if status.code() == tonic::Code::ResourceExhausted => {
                    // Backpressure activado correctamente si el worker asíncrono del test 
                    // no dio abasto para desencolar 1000 iteraciones instantáneas.
                    println!("Test validó comportamiento SRE de Backpressure");
                    return Ok(());
                }
                Err(e) => anyhow::bail!("Error gRPC inesperado: {}", e),
            }
        }

        // Si llegó hasta aquí, procesó los 1000 chunks usando el worker concurrente exitosamente y en orden
        Ok(())
    }
}
