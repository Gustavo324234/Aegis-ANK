use ank_core::{enclave::master::MasterEnclave, SchedulerEvent, PCB as CorePCB};
use ank_proto::v1::kernel_service_server::KernelService;
use ank_proto::v1::{
    AdminSetupRequest, AdminSetupResponse, Empty, PasswordResetRequest, Priority as ProtoPriority,
    ProcessList, SystemState, SystemStatus, TaskEvent, TaskRequest, TaskResponse,
    TaskSubscription, TenantCreateRequest, TenantCreateResponse, Pcb as ProtoPcb,
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
    master_enclave: MasterEnclave,
}

impl AnkRpcServer {
    pub fn new(
        scheduler_tx: mpsc::Sender<SchedulerEvent>,
        event_broker: Arc<RwLock<HashMap<String, mpsc::Sender<TaskEvent>>>>,
        master_enclave: MasterEnclave,
    ) -> Self {
        Self {
            scheduler_tx,
            event_broker,
            master_enclave,
        }
    }
}

#[tonic::async_trait]
impl KernelService for AnkRpcServer {
    async fn submit_task(
        &self,
        request: Request<TaskRequest>,
    ) -> Result<Response<TaskResponse>, Status> {
        // Extraction of Multi-Tenant context from gRPC Extensions (injected by Interceptor)
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        let req = request.into_inner();

        if req.prompt.is_empty() {
            return Err(Status::invalid_argument("Prompt cannot be empty"));
        }

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
            .cloned()
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
        request: Request<Empty>,
    ) -> Result<Response<SystemStatus>, Status> {
        // Validation of Multi-Tenant context
        let _auth = request
            .extensions()
            .get::<CitadelAuth>()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        // Determinar estado basado en si el Master Admin está inicializado
        let is_init = self.master_enclave.is_initialized().await
            .map_err(|e| Status::internal(format!("DB Error: {}", e)))?;
        
        let state = if is_init {
            SystemState::StateOperational as i32
        } else {
            SystemState::StateInitializing as i32
        };

        // En esta fase, devolvemos valores mockeados o telemetría básica
        Ok(Response::new(SystemStatus {
            cpu_load: 0.15,
            vram_allocated_mb: 1024.0,
            vram_total_mb: 8192.0,
            total_processes: 1,
            active_workers: 1,
            uptime: "00:01:00".to_string(),
            loaded_models: vec!["Llama-3-8B-Instruct".to_string()],
            state,
        }))
    }

    async fn list_processes(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ProcessList>, Status> {
        // Validation of Multi-Tenant context
        let _auth = request
            .extensions()
            .get::<CitadelAuth>()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

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
        // Extraction of Multi-Tenant context from gRPC Extensions
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        let pcb = request.into_inner();
        let pid = pcb.pid.clone();

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

    async fn initialize_master_admin(
        &self,
        request: Request<AdminSetupRequest>,
    ) -> Result<Response<AdminSetupResponse>, Status> {
        let req = request.into_inner();

        match self.master_enclave.initialize_master(&req.username, &req.passphrase).await {
            Ok(_) => Ok(Response::new(AdminSetupResponse {
                success: true,
                message: "Master Admin successfully initialized".to_string(),
            })),
            Err(e) => Err(Status::already_exists(e.to_string())),
        }
    }

    async fn create_tenant(
        &self,
        request: Request<TenantCreateRequest>,
    ) -> Result<Response<TenantCreateResponse>, Status> {
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned() // Clonamos inmediatamente para desligar de la vida de request
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        let req = request.into_inner();

        // Validar que la request venga del root
        let is_authed = self.master_enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if !is_authed {
            return Err(Status::permission_denied("Only Master Admin can create tenants"));
        }
        
        match self.master_enclave.create_tenant(&req.username).await {
            Ok((port, pass)) => Ok(Response::new(TenantCreateResponse {
                success: true,
                tenant_id: req.username,
                temporary_passphrase: pass,
                network_port: port,
                message: "Tenant created".to_string(),
            })),
            Err(e) => Err(Status::internal(format!("Failed to create tenant: {}", e))),
        }
    }

    async fn reset_tenant_password(
        &self,
        request: Request<PasswordResetRequest>,
    ) -> Result<Response<Empty>, Status> {
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned() // Clonamos inmediatamente para desligar de la vida de request
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        let req = request.into_inner();

        // Validar que sea un Master Admin autorizado o el propio usuario reseteando su password?
        // En Citadel, normalmente un Master Admin o un servicio de recu puede forzarlo.
        let is_master = self.master_enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
            .unwrap_or(false);

        // Para simplificar, requerimos Master o que el propio usuario provea auth validado del tenant.
        // Asumimos root por ahora.
        if !is_master && auth.tenant_id != req.tenant_id {
            return Err(Status::permission_denied("No permission to reset this tenant password"));
        }

        self.master_enclave
            .reset_tenant_password(&req.tenant_id, &req.new_passphrase)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }
}
