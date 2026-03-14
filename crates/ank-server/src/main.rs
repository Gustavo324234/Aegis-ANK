use ank_core::plugins::watcher::watch_plugins_dir;
use ank_core::plugins::PluginManager;
use ank_core::{
    enclave::master::MasterEnclave, CognitiveScheduler, SQLCipherPersistor, SchedulerEvent,
    StatePersistor,
};
use ank_proto::v1::kernel_service_server::KernelServiceServer;
use anyhow::Context;
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

    // Rule ANK-910: Pre-validate and initialize privileged regexes
    ank_core::syscalls::init_syscall_regexes();

    // Canales de comunicación del bus de eventos entre gRPC y el Scheduler Cognitivo
    let (scheduler_tx, scheduler_rx) = mpsc::channel::<SchedulerEvent>(100);
    // Clon local del sender para inyección de dependencias distribuidas
    let internal_tx = scheduler_tx.clone();

    // El Event Broker gestiona los streams de output hacia los clientes gRPC
    let event_broker = Arc::new(RwLock::new(HashMap::new()));

    // Regla ANK-2412: Inicializar PersistenceManager (SQLCipher)
    // Usamos AEGIS_ROOT_KEY (mismo que Enclave) para cifrar la persistencia del Scheduler
    let root_key = std::env::var("AEGIS_ROOT_KEY")
        .context("FATAL: AEGIS_ROOT_KEY environment variable is missing.")?;

    let persistence = Arc::new(SQLCipherPersistor::new("scheduler_state.db", &root_key)?);

    // Inicializar el Cognitive Scheduler principal con persistencia inyectada
    let scheduler = CognitiveScheduler::new(Arc::clone(&persistence) as Arc<dyn StatePersistor>);

    // SIGTERM / Ctrl+C Handler: Atomic Flush
    let persistence_for_signal = Arc::clone(&persistence);
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            info!("SIGTERM/Ctrl+C received. Flushing persistence layer...");
            if let Err(e) = persistence_for_signal.flush().await {
                tracing::error!("Final flush failed: {}", e);
            }
            info!("Flush complete. Terminating ANK server safely.");
            std::process::exit(0);
        }
    });

    // Hot-reload Wasm Plugins Initialization
    let plugin_manager = Arc::new(RwLock::new(PluginManager::new()?));
    let pm_clone = Arc::clone(&plugin_manager);

    // Spawn zero-downtime hot-reload daemon
    tokio::spawn(async move {
        if let Err(e) = watch_plugins_dir("./plugins".to_string(), pm_clone).await {
            tracing::error!("Failed to start Wasm Hot-Reload watcher: {}", e);
        }
    });

    info!("Iniciando hilo de ejecución principal (Cognitive Scheduler)...");
    tokio::spawn(async move {
        // start requiere el Receiver pasivo y el Sender para loops internos/Teleport
        if let Err(e) = scheduler.start(scheduler_rx, internal_tx).await {
            tracing::error!("Scheduler loop crashed: {}", e);
        }
    });

    // Instanciar el Master Enclave (DB administrativa)
    let master_enclave = MasterEnclave::open("admin.db", &root_key).await?;

    // Configuración e instanciación del servidor gRPC (0.0.0.0:50051 per req)
    let addr = "0.0.0.0:50051".parse()?;

    let pm_clone2 = Arc::clone(&plugin_manager);
    let hal = Arc::new(RwLock::new(ank_core::chal::CognitiveHAL::new(pm_clone2)));

    // Instanciar el servicio con la UI / Cliente Python apuntando acá
    let ank_service = AnkRpcServer::new(
        scheduler_tx.clone(),
        Arc::clone(&event_broker),
        master_enclave,
        hal,
    );

    // Aplicar Middleware de Autenticación (Citadel Protocol)
    let svc =
        KernelServiceServer::with_interceptor(ank_service, ank_server::server::auth_interceptor);

    // Servicio Siren
    let siren_impl =
        ank_server::siren::AnkSirenService::new(scheduler_tx.clone(), Arc::clone(&event_broker))?;
    let siren_svc =
        ank_proto::v1::siren::siren_service_server::SirenServiceServer::with_interceptor(
            siren_impl,
            ank_server::server::auth_interceptor,
        );

    info!(
        "ANK KernelService y SirenService levantados exitosamente en {}",
        addr
    );

    // Levantar Tonic Server
    Server::builder()
        .add_service(svc)
        .add_service(siren_svc)
        .serve(addr)
        .await?;

    Ok(())
}
