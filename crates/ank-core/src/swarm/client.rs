use std::time::Duration;
use tokio::sync::mpsc;
use tonic::transport::Endpoint;
use tracing::{info, warn};
use thiserror::Error;

use crate::pcb::PCB;
use crate::scheduler::SchedulerEvent;
use ank_proto::v1::kernel_service_client::KernelServiceClient;
use ank_proto::v1::{Pcb, ProcessState as ProtoProcessState};

#[derive(Error, Debug)]
pub enum SwarmError {
    #[error("Connection refused for node {0}:{1}")]
    ConnectionRefused(String, u16),
    
    #[error("Transport error: {0}")]
    TransportError(#[from] tonic::transport::Error),
    
    #[error("RPC error: {0}")]
    RpcError(#[from] tonic::Status),

    #[error("Teleportation timeout")]
    Timeout,

    #[error("Internal conversion error: {0}")]
    ConversionError(String),
}

/// Cliente gRPC para la teletransportación de procesos entre nodos del Swarm.
pub struct SwarmClient;

impl SwarmClient {
    /// Teletransporta un PCB a un nodo remoto.
    /// Inicia un stream de eventos que se re-inyectan en el Scheduler local.
    pub async fn teleport(
        &self,
        target_ip: &str,
        target_port: u16,
        pcb: PCB,
        event_tx: mpsc::Sender<SchedulerEvent>,
    ) -> Result<(), SwarmError> {
        let uri = format!("http://{}:{}", target_ip, target_port);
        
        // Configuración del endpoint con timeouts estrictos para evitar bloqueos del Kernel
        let endpoint = Endpoint::from_shared(uri)?
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(30)); // 30s de timeout para la ejecución total del RPC

        info!("Connecting to target node for teleportation: {}:{}", target_ip, target_port);
        
        let mut client = KernelServiceClient::connect(endpoint)
            .await
            .map_err(|_| SwarmError::ConnectionRefused(target_ip.to_string(), target_port))?;

        // Conversión del PCB nativo al formato Protobuf
        let proto_pcb = self.convert_pcb_to_proto(&pcb)?;
        let remote_pid = pcb.pid.clone();

        info!(pid = %remote_pid, "Initiating PCB teleportation...");

        // Llamada RPC para teletransportar el proceso
        let response = client.teleport_process(proto_pcb).await?;
        let mut stream = response.into_inner();

        // Bucle de recepción del Stream de eventos.
        // Los eventos recibidos del Worker se re-inyectan en el Scheduler del Host.
        tokio::spawn(async move {
            info!(pid = %remote_pid, "Receiving teleported process events...");
            
            while let Ok(Some(event)) = stream.message().await {
                // Mapear el `TaskEvent` de Protobuf re-inyectándolo en el event_tx del Scheduler local
                info!(pid = %remote_pid, "Event received from remote: {:?}", event);
                
                let local_event = SchedulerEvent::RemoteEvent(remote_pid.clone(), event);
                if event_tx.send(local_event).await.is_err() {
                    warn!(pid = %remote_pid, "Scheduler event receiver closed. Dropping remote events.");
                    break;
                }
            }
            
            warn!(pid = %remote_pid, "Teleported process stream ended.");
        });

        Ok(())
    }

    /// Helper para convertir la estructura interna de ANK al contrato de Protobuf.
    fn convert_pcb_to_proto(&self, pcb: &PCB) -> Result<Pcb, SwarmError> {
        // Mapeo manual de campos (SRE: No usamos macros mágicas que puedan fallar en runtime)
        Ok(Pcb {
            pid: pcb.pid.clone(),
            parent_pid: pcb.parent_pid.clone().unwrap_or_default(),
            state: match pcb.state {
                crate::pcb::ProcessState::New => ProtoProcessState::StatePending.into(),
                crate::pcb::ProcessState::Ready => ProtoProcessState::StatePending.into(),
                crate::pcb::ProcessState::Running => ProtoProcessState::StateRunning.into(),
                crate::pcb::ProcessState::WaitingSyscall => ProtoProcessState::StateBlocked.into(),
                crate::pcb::ProcessState::Completed => ProtoProcessState::StateCompleted.into(),
                crate::pcb::ProcessState::Failed => ProtoProcessState::StateTerminated.into(),
            },
            quantum_used: pcb.execution_metrics.cycles_executed,
            memory: Some(ank_proto::v1::pcb::MemorySpace {
                instruction_pointer: pcb.program_counter.current_node.clone(),
                context_refs: pcb.memory_pointers.l2_context_refs.clone(),
                registers: pcb.registers.temp_vars.clone(),
            }),
            inlined_context: pcb.inlined_context.clone(),
            created_at: Some(prost_types::Timestamp {
                seconds: pcb.created_at.timestamp(),
                nanos: pcb.created_at.timestamp_subsec_nanos() as i32,
            }),
            last_updated: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: Utc::now().timestamp_subsec_nanos() as i32,
            }),
            priority: pcb.priority,
            process_name: pcb.process_name.clone(),
            tenant_id: pcb.tenant_id.clone().unwrap_or_default(),
            session_key: pcb.session_key.clone().unwrap_or_default(),
        })
    }
}

use chrono::Utc;
