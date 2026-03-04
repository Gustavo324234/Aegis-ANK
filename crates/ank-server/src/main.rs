use ank_core::{enclave::master::MasterEnclave, CognitiveScheduler, SchedulerEvent};
use ank_proto::v1::kernel_service_server::KernelServiceServer;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tonic::transport::Server;
use tracing::info;

use ank_server::server::AnkRpcServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Inicializar logging para ver métricas y trazas en consola
    tracing_subscriber::fmt::init();

    info!("Aegis Neural Kernel (ANK) System Booting...");

    // Canales de comunicación del bus de eventos entre gRPC y el Scheduler Cognitivo
    let (scheduler_tx, scheduler_rx) = mpsc::channel::<SchedulerEvent>(100);
    // Clon local del sender para inyección de dependencias distribuidas
    let internal_tx = scheduler_tx.clone();

    // El Event Broker gestiona los streams de output hacia los clientes gRPC
    let event_broker = Arc::new(RwLock::new(HashMap::new()));

    // Inicializar el Cognitive Scheduler principal
    let scheduler = CognitiveScheduler::new();

    info!("Iniciando hilo de ejecución principal (Cognitive Scheduler)...");
    tokio::spawn(async move {
        // start requiere el Receiver pasivo y el Sender para loops internos/Teleport
        if let Err(e) = scheduler.start(scheduler_rx, internal_tx).await {
            tracing::error!("Scheduler loop crashed: {}", e);
        }
    });

    // Instanciar el Master Enclave (DB administrativa)
    // El 'root_key' en producción debería ser inyectado vía variable de entorno.
    let root_key = std::env::var("AEGIS_ROOT_KEY").unwrap_or_else(|_| "default_root_key".to_string());
    let master_enclave = MasterEnclave::open("admin.db", &root_key).await?;

    // Configuración e instanciación del servidor gRPC (0.0.0.0:50051 per req)
    let addr = "0.0.0.0:50051".parse()?;
    
    // Instanciar el servicio con la UI / Cliente Python apuntando acá
    let ank_service = AnkRpcServer::new(scheduler_tx, Arc::clone(&event_broker), master_enclave);

    // Aplicar Middleware de Autenticación (Citadel Protocol)
    let svc = KernelServiceServer::with_interceptor(
        ank_service,
        ank_server::server::auth_interceptor,
    );

    info!("ANK KernelService levantado exitosamente en {}", addr);

    // Levantar Tonic Server
    Server::builder().add_service(svc).serve(addr).await?;

    Ok(())
}
