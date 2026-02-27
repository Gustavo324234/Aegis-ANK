use ank_core::{Scheduler, SchedulerEvent};
use ank_proto::v1::kernel_service_server::KernelServiceServer;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tonic::transport::Server;
use tracing::info;

use ank_server::server::AnkRpcServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Inicializar logging
    tracing_subscriber::fmt::init();

    info!("Aegis Neural Kernel (ANK) Bridge starting...");

    // Canales de comunicación entre gRPC y Scheduler
    let (scheduler_tx, scheduler_rx) = mpsc::channel::<SchedulerEvent>(100);
    
    // El Event Broker gestiona los streams de output hacia los clientes
    let event_broker = Arc::new(RwLock::new(HashMap::new()));
    
    // Inicializar el Scheduler en background
    let scheduler = Arc::new(RwLock::new(Scheduler::new()));
    let scheduler_clone = Arc::clone(&scheduler);
    
    info!("Starting Cognitive Scheduler loop...");
    tokio::spawn(async move {
        Scheduler::run(scheduler_clone, scheduler_rx).await;
    });

    // Configuración del servidor gRPC
    // TODO: Hacer el puerto configurable vía env var en futuras versiones
    let addr = "127.0.0.1:50051".parse()?;
    let ank_service = AnkRpcServer::new(scheduler_tx, Arc::clone(&event_broker));

    info!("ANK-Bridge listening on {}", addr);

    Server::builder()
        .add_service(KernelServiceServer::new(ank_service))
        .serve(addr)
        .await?;

    Ok(())
}
