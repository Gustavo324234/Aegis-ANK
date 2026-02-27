use ank_core::{PCB as CorePCB, SchedulerEvent};
use ank_proto::v1::kernel_service_server::KernelService;
use ank_proto::v1::{TaskRequest, TaskResponse, TaskSubscription, TaskEvent, SystemStatus, Empty, Priority as ProtoPriority};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};
use tracing::info;

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

        let priority = match req.priority() {
            ProtoPriority::Low => 0,
            ProtoPriority::Medium => 5,
            ProtoPriority::High => 10,
        };

        // Crear el PCB en ank-core
        let core_pcb = CorePCB::new(
            "Remote Task".to_string(),
            priority,
            req.prompt,
        );
        
        let pid = core_pcb.pid.clone();

        info!("Received SubmitTask: {} (Priority: {})", pid, priority);

        // Enviar al Scheduler
        if let Err(e) = self.scheduler_tx.send(SchedulerEvent::RegisterProcess(core_pcb)).await {
            return Err(Status::internal(format!("Failed to register task: {}", e)));
        }

        Ok(Response::new(TaskResponse {
            pid,
            accepted: true,
        }))
    }

    type WatchTaskStream = ReceiverStream<Result<TaskEvent, Status>>;

    async fn watch_task(
        &self,
        request: Request<TaskSubscription>,
    ) -> Result<Response<Self::WatchTaskStream>, Status> {
        let req = request.into_inner();
        let pid = req.pid;

        info!("Client subscribing to events for PID: {}", pid);

        let (tx, rx) = mpsc::channel(100);
        
        {
            let mut broker = self.event_broker.write().await;
            broker.insert(pid.clone(), tx);
        }

        let stream = ReceiverStream::new(rx).map(Ok);
        Ok(Response::new(stream))
    }

    async fn get_system_status(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<SystemStatus>, Status> {
        // En esta fase, devolvemos valores mockeados o telemetría básica
        Ok(Response::new(SystemStatus {
            cpu_usage: 0.15,
            vram_usage: 0.42,
            active_model: "Llama-3-8B-Instruct (Mocked)".to_string(),
            is_local_inference_ready: true,
        }))
    }
}
