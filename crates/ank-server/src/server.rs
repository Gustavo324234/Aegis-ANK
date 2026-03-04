use ank_core::{SchedulerEvent, PCB as CorePCB};
use ank_proto::v1::kernel_service_server::KernelService;
use ank_proto::v1::{
    Empty, Priority as ProtoPriority, ProcessList, SystemStatus, TaskEvent, TaskRequest,
    TaskResponse, TaskSubscription, Pcb as ProtoPcb,
};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

#[derive(Clone)]
pub struct CitadelAuth {
    pub tenant_id: String,
    pub session_key: String,
}

impl std::fmt::Debug for CitadelAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CitadelAuth")
            .field("tenant_id", &self.tenant_id)
            .field("session_key", &"***REDACTED***")
            .finish()
    }
}

pub fn auth_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    let metadata = req.metadata();

    let tenant_id = metadata
        .get("x-aegis-tenant-id")
        .ok_or_else(|| Status::unauthenticated("Citadel Protocol: Missing x-aegis-tenant-id"))?
        .to_str()
        .map_err(|_| Status::unauthenticated("Invalid tenant_id format"))?
        .to_string();

    let session_key = metadata
        .get("x-aegis-session-key")
        .ok_or_else(|| Status::unauthenticated("Citadel Protocol: Missing x-aegis-session-key"))?
        .to_str()
        .map_err(|_| Status::unauthenticated("Invalid session_key format"))?
        .to_string();

    let mut req = req;
    req.extensions_mut().insert(CitadelAuth {
        tenant_id,
        session_key,
    });

    Ok(req)
}

pub struct AnkRpcServer {
    scheduler_tx: mpsc::Sender<SchedulerEvent>,
    event_broker: Arc<RwLock<HashMap<String, mpsc::Sender<TaskEvent>>>>,
}

impl AnkRpcServer {
    pub fn new(
        scheduler_tx: mpsc::Sender<SchedulerEvent>,
        event_broker: Arc<RwLock<HashMap<String, mpsc::Sender<TaskEvent>>>>,
    ) -> Self {
        Self {
            scheduler_tx,
            event_broker,
        }
    }
}

#[tonic::async_trait]
impl KernelService for AnkRpcServer {
    async fn submit_task(
        &self,
        request: Request<TaskRequest>,
    ) -> Result<Response<TaskResponse>, Status> {
        let req = request.into_inner();

        if req.prompt.is_empty() {
            return Err(Status::invalid_argument("Prompt cannot be empty"));
        }

        // Extraction of Multi-Tenant context from gRPC Extensions (injected by Interceptor)
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        // Mapping proto priority to core priority
        let priority = match req.priority() {
            ProtoPriority::Idle => 0,
            ProtoPriority::Low => 1,
            ProtoPriority::Normal => 5,
            ProtoPriority::Critical => 10,
        };

        // Crear el PCB en ank-core
        let mut core_pcb = CorePCB::new("Remote Task".to_string(), priority, req.prompt);
        core_pcb.tenant_id = Some(auth.tenant_id.clone());
        core_pcb.session_key = Some(auth.session_key.clone());

        let pid = core_pcb.pid.clone();

        info!(
            "Received SubmitTask: {} (Tenant: {}, Priority: {})",
            pid, auth.tenant_id, priority
        );

        // Enviar al Scheduler
        if let Err(e) = self
            .scheduler_tx
            .send(SchedulerEvent::ScheduleTask(Box::new(core_pcb)))
            .await
        {
            return Err(Status::internal(format!("Failed to register task: {}", e)));
        }

        Ok(Response::new(TaskResponse {
            pid,
            accepted: true,
            message: "Task successfully submitted to Cognitive Scheduler".to_string(),
        }))
    }

    type WatchTaskStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<TaskEvent, Status>> + Send>>;

    async fn watch_task(
        &self,
        request: Request<TaskSubscription>,
    ) -> Result<Response<Self::WatchTaskStream>, Status> {
        // Validation of Multi-Tenant context from gRPC Extensions
        let _auth = request
            .extensions()
            .get::<CitadelAuth>()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        let req = request.into_inner();
        let pid = req.pid;

        info!("Client subscribing to events for PID: {}", pid);

        let (tx, rx) = mpsc::channel(100);

        {
            let mut broker = self.event_broker.write().await;
            broker.insert(pid.clone(), tx);
        }

        let stream = ReceiverStream::new(rx).map(Ok);
        Ok(Response::new(Box::pin(stream) as Self::WatchTaskStream))
    }

    async fn get_system_status(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<SystemStatus>, Status> {
        // En esta fase, devolvemos valores mockeados o telemetría básica
        Ok(Response::new(SystemStatus {
            cpu_load: 0.15,
            vram_allocated_mb: 1024.0,
            vram_total_mb: 8192.0,
            total_processes: 1,
            active_workers: 1,
            uptime: "00:01:00".to_string(),
            loaded_models: vec!["Llama-3-8B-Instruct".to_string()],
        }))
    }

    async fn list_processes(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ProcessList>, Status> {
        // Implementación pendiente del Scheduler
        Ok(Response::new(ProcessList {
            processes: Vec::new(),
        }))
    }

    type TeleportProcessStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<TaskEvent, Status>> + Send>>;

    async fn teleport_process(
        &self,
        request: Request<ProtoPcb>,
    ) -> Result<Response<Self::TeleportProcessStream>, Status> {
        let pcb = request.into_inner();
        let pid = pcb.pid.clone();

        // Extraction of Multi-Tenant context from gRPC Extensions
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        // Convertir ProtoPcb a CorePCB (Mapeo básico por ahora)
        let mut core_pcb = CorePCB::new(
            pcb.process_name,
            pcb.priority,
            pcb.memory
                .as_ref()
                .map(|m| m.instruction_pointer.clone())
                .unwrap_or_default(),
        );
        core_pcb.pid = pid.clone();
        core_pcb.tenant_id = Some(auth.tenant_id.clone());
        core_pcb.session_key = Some(auth.session_key.clone());
        core_pcb.parent_pid = if pcb.parent_pid.is_empty() {
            None
        } else {
            Some(pcb.parent_pid)
        };
        core_pcb.inlined_context = pcb.inlined_context;

        // Suscribirse a eventos ANTES de enviar al scheduler para no perder nada
        let (tx, rx) = mpsc::channel(100);
        {
            let mut broker = self.event_broker.write().await;
            broker.insert(pid.clone(), tx);
        }

        // Enviar al Scheduler
        if let Err(e) = self
            .scheduler_tx
            .send(SchedulerEvent::ScheduleTask(Box::new(core_pcb)))
            .await
        {
            warn!("Failed to schedule teleported task {}: {}", pid, e);
            return Err(Status::internal(format!("Failed to schedule task: {}", e)));
        }

        let stream = ReceiverStream::new(rx).map(Ok);
        Ok(Response::new(Box::pin(stream) as Self::TeleportProcessStream))
    }
}
