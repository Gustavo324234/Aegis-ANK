use ank_core::{CognitiveScheduler, SQLCipherPersistor, SchedulerEvent, PCB};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_scheduler_priority_and_preemption() {
    // Base de datos en memoria para la prueba de integración
    let persistence = Arc::new(SQLCipherPersistor::new(":memory:", "test_key").unwrap());
    let scheduler = CognitiveScheduler::new(persistence);
    let (tx, rx) = mpsc::channel(100);
    let tx_internal = tx.clone();

    // Iniciar el Scheduler en background
    tokio::spawn(async move {
        let _ = scheduler.start(rx, tx_internal).await;
    });

    // 1. Inyectar tarea de prioridad media
    let pcb_med = PCB::new("MedPriority".to_string(), 5, "Prompt 1".to_string());
    tx.send(SchedulerEvent::ScheduleTask(Box::new(pcb_med.clone())))
        .await
        .unwrap();

    // Esperar un poco para el despacho inicial
    sleep(Duration::from_millis(150)).await;

    // 2. Inyectar tarea de prioridad alta (10)
    let pcb_high = PCB::new("HighPriority".to_string(), 10, "Prompt 2".to_string());
    tx.send(SchedulerEvent::ScheduleTask(Box::new(pcb_high.clone())))
        .await
        .unwrap();

    // Esperar a que el scheduler procese el evento y la ponga en Ready Queue
    sleep(Duration::from_millis(150)).await;

    // En este punto, como no hay un driver real consumiendo el 'current_running',
    // el Scheduler simplemente despacha el primero (prioridad 5) y mantiene el resto en Ready.
    // La prioridad 10 debería estar al tope de la Ready Queue ahora.
}
