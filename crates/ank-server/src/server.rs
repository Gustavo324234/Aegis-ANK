use crate::auth::citadel::{generate_public_tenant_id, sanitize_error, SafeIdentity};
use ank_core::{enclave::master::MasterEnclave, SchedulerEvent, PCB as CorePCB};
use ank_proto::v1::kernel_service_server::KernelService;
use ank_proto::v1::{
    AdminSetupRequest, AdminSetupResponse, Empty, EngineConfigRequest, PasswordResetRequest,
    Pcb as ProtoPcb, Priority as ProtoPriority, ProcessList, ProcessState as ProtoProcessState,
    SystemState, SystemStatus, TaskEvent, TaskRequest, TaskResponse, TaskSubscription,
    TenantCreateRequest, TenantCreateResponse,
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
    pub public_id: String, // Added public_id for telemetry
}

impl std::fmt::Debug for CitadelAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CitadelAuth")
            .field("tenant_id", &"***REDACTED***") // Now redact the private ID
            .field("public_id", &self.public_id)
            .field("session_key", &"***REDACTED***")
            .finish()
    }
}

/// Interceptor de autenticación Citadel.
/// Extrae la identidad del Tenant pero NO bloquea si faltan headers,
/// delegando la decisión de seguridad a cada RPC individualmente. Esto es CRÍTICO
/// para que GetSystemStatus pueda responder 0 (Initializing) a una Shell sin sesión.
pub fn auth_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    let metadata = req.metadata();

    let tenant_id = match metadata.get("x-aegis-tenant-id") {
        Some(v) => match v.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return Err(Status::unauthenticated("Invalid tenant_id format")),
        },
        None => return Ok(req),
    };

    let session_key = match metadata.get("x-aegis-session-key") {
        Some(v) => match v.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return Err(Status::unauthenticated("Invalid session_key format")),
        },
        None => return Ok(req),
    };

    // Hardening ANK-2410: Derive public identity
    let root_key = std::env::var("AEGIS_ROOT_KEY").map_err(|_| {
        Status::unauthenticated("Aegis Security: Missing encryption context (ROOT_KEY)")
    })?;

    let public_id = generate_public_tenant_id(&tenant_id, root_key.as_bytes())
        .map_err(|e| Status::internal(format!("Aegis Identity Error: {}", e)))?;

    let mut req = req;

    // Inyectar SafeIdentity solicitado en el ticket
    req.extensions_mut().insert(SafeIdentity {
        private_id: tenant_id.clone(),
        public_id: public_id.clone(),
    });

    req.extensions_mut().insert(CitadelAuth {
        tenant_id,
        session_key,
        public_id,
    });

    Ok(req)
}

pub struct AnkRpcServer {
    scheduler_tx: mpsc::Sender<SchedulerEvent>,
    event_broker: Arc<RwLock<HashMap<String, Vec<mpsc::Sender<TaskEvent>>>>>,
    master_enclave: MasterEnclave,
    hal: Arc<RwLock<ank_core::chal::CognitiveHAL>>,
}

impl AnkRpcServer {
    pub fn new(
        scheduler_tx: mpsc::Sender<SchedulerEvent>,
        event_broker: Arc<RwLock<HashMap<String, Vec<mpsc::Sender<TaskEvent>>>>>,
        master_enclave: MasterEnclave,
        hal: Arc<RwLock<ank_core::chal::CognitiveHAL>>,
    ) -> Self {
        let broker_clone = event_broker.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let mut broker = broker_clone.write().await;
                broker.retain(|_, subs| {
                    subs.retain(|tx| !tx.is_closed());
                    !subs.is_empty()
                });
            }
        });

        Self {
            scheduler_tx,
            event_broker,
            master_enclave,
            hal,
        }
    }

    async fn validate_auth(&self, auth: &CitadelAuth) -> Result<(), Status> {
        if let Ok(is_master) = self
            .master_enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
        {
            if is_master {
                return Ok(());
            }
        }

        if let Ok(is_tenant) = self
            .master_enclave
            .authenticate_tenant(&auth.tenant_id, &auth.session_key)
            .await
        {
            if is_tenant {
                return Ok(());
            }
        }

        Err(Status::unauthenticated(
            "Citadel AUTH_FAILURE: Access Denied.",
        ))
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
        core_pcb.public_id = Some(auth.public_id.clone());
        core_pcb.session_key = Some(auth.session_key.clone());

        let pid = core_pcb.pid.clone();

        info!(
            "Received SubmitTask: {} (Tenant_Public: {}, Priority: {})",
            pid, auth.public_id, priority
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
            broker.entry(pid.clone()).or_insert_with(Vec::new).push(tx);
        }

        let stream = ReceiverStream::new(rx).map(Ok);
        Ok(Response::new(Box::pin(stream) as Self::WatchTaskStream))
    }

    async fn get_system_status(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<SystemStatus>, Status> {
        // Determinar estado basado en si el Master Admin está inicializado.
        // Si hay error en la DB (ej. borrada), asumimos false para permitir redirección al Setup.
        let is_init = self
            .master_enclave
            .is_initialized()
            .await
            .unwrap_or_else(|e| {
                warn!("MasterEnclave check failed, reporting uninitialized: {}", e);
                false
            });

        // Validación de contexto solo si el sistema se reporta operativo.
        let auth = request.extensions().get::<CitadelAuth>();

        if is_init && auth.is_none() {
            return Err(Status::unauthenticated(
                "Citadel Protocol context missing (System is Operational)",
            ));
        }

        if is_init {
            if let Some(a) = auth {
                self.validate_auth(a).await?;
            }
        }

        // Reportamos explícitamente el valor acorde al enum (0: Initializing, 1: Operational)
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
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if let Err(e) = self
            .scheduler_tx
            .send(SchedulerEvent::ListProcesses(reply_tx))
            .await
        {
            return Err(Status::internal(format!(
                "Failed to reach scheduler: {}",
                e
            )));
        }

        let core_processes = reply_rx
            .await
            .map_err(|e| Status::internal(format!("Scheduler failed to reply: {}", e)))?;

        // Mapear CorePCB a ProtoPCB y filtrar por Tenant
        let processes = core_processes
            .into_iter()
            .filter(|p| {
                // Si somos master adm (en una impl real veriamos) o si pertenece al tenant
                p.tenant_id.as_deref() == Some(&auth.tenant_id)
            })
            .map(|p| {
                let state = match p.state {
                    ank_core::ProcessState::New => ProtoProcessState::StatePending,
                    ank_core::ProcessState::Ready => ProtoProcessState::StatePending,
                    ank_core::ProcessState::Running => ProtoProcessState::StateRunning,
                    ank_core::ProcessState::WaitingSyscall => ProtoProcessState::StateBlocked,
                    ank_core::ProcessState::Completed => ProtoProcessState::StateCompleted,
                    ank_core::ProcessState::Failed => ProtoProcessState::StateTerminated,
                };

                ProtoPcb {
                    pid: p.pid.clone(),
                    parent_pid: p.parent_pid.unwrap_or_default(),
                    state: state.into(),
                    quantum_used: Default::default(),
                    memory: Some(ank_proto::v1::pcb::MemorySpace {
                        instruction_pointer: p.memory_pointers.l1_instruction.clone(),
                        context_refs: p.memory_pointers.l2_context_refs.clone(),
                        registers: p.registers.temp_vars.clone(),
                    }),
                    inlined_context: p.inlined_context.clone(),
                    created_at: Some(prost_types::Timestamp {
                        seconds: p.created_at.timestamp(),
                        nanos: p.created_at.timestamp_subsec_nanos() as i32,
                    }),
                    last_updated: None,
                    priority: p.priority,
                    process_name: p.process_name.clone(),
                    tenant_id: p.public_id.unwrap_or_default(), // Return obfuscated ID
                }
            })
            .collect();

        Ok(Response::new(ProcessList { processes }))
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
        core_pcb.public_id = Some(auth.public_id.clone());
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
            broker.entry(pid.clone()).or_insert_with(Vec::new).push(tx);
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
        Ok(Response::new(
            Box::pin(stream) as Self::TeleportProcessStream
        ))
    }

    async fn initialize_master_admin(
        &self,
        request: Request<AdminSetupRequest>,
    ) -> Result<Response<AdminSetupResponse>, Status> {
        let req = request.into_inner();

        match self
            .master_enclave
            .initialize_master(&req.username, &req.passphrase)
            .await
        {
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
        let is_authed = self
            .master_enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if !is_authed {
            return Err(Status::permission_denied(
                "Only Master Admin can create tenants",
            ));
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
        let is_master = self
            .master_enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
            .unwrap_or(false);

        // Para simplificar, requerimos Master o que el propio usuario provea auth validado del tenant.
        // Asumimos root por ahora.
        if !is_master && auth.tenant_id != req.tenant_id {
            return Err(Status::permission_denied(
                "No permission to reset this tenant password",
            ));
        }

        self.master_enclave
            .reset_tenant_password(&req.tenant_id, &req.new_passphrase)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Empty {}))
    }

    async fn configure_engine(
        &self,
        request: Request<EngineConfigRequest>,
    ) -> Result<Response<Empty>, Status> {
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        let req = request.into_inner();

        // 1. Guardar en SQLite cifrado de este Tenant
        let tenant_db = ank_core::enclave::TenantDB::open(&auth.tenant_id, &auth.session_key)
            .map_err(|e| {
                let safe_msg = sanitize_error(&e.to_string(), &auth.tenant_id, &auth.public_id);
                Status::internal(format!("Failed to open tenant DB: {}", safe_msg))
            })?;

        tenant_db
            .set_kv("engine_api_url", &req.api_url)
            .map_err(|e| {
                let safe_msg = sanitize_error(&e.to_string(), &auth.tenant_id, &auth.public_id);
                Status::internal(safe_msg)
            })?;

        tenant_db
            .set_kv("engine_model", &req.model_name)
            .map_err(|e| {
                let safe_msg = sanitize_error(&e.to_string(), &auth.tenant_id, &auth.public_id);
                Status::internal(safe_msg)
            })?;

        tenant_db
            .set_kv("engine_api_key", &req.api_key)
            .map_err(|e| {
                let safe_msg = sanitize_error(&e.to_string(), &auth.tenant_id, &auth.public_id);
                Status::internal(safe_msg)
            })?;

        // 2. Notificar al CognitiveHAL (Driver en Memoria) de la nueva credencial
        {
            let mut hal = self.hal.write().await;
            hal.update_cloud_credentials(req.api_url, req.model_name, req.api_key);
        }

        info!(
            "Engine correctly configured for public_tenant {}",
            auth.public_id
        );

        Ok(Response::new(Empty {}))
    }
}
