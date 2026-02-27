use ank_core::{PCB, Scheduler, SchedulerEvent};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_scheduler_priority_and_preemption() {
    let scheduler = Arc::new(RwLock::new(Scheduler::new()));
    let (tx, rx) = mpsc::channel(100);

    // Iniciar el Scheduler en un hilo separado
    let scheduler_clone = Arc::clone(&scheduler);
    tokio::spawn(async move {
        Scheduler::run(scheduler_clone, rx).await;
    });

    // 1. Inyectar tarea de prioridad media
    let pcb_med = PCB::new("MedPriority".to_string(), 5, "Prompt 1".to_string());
    tx.send(SchedulerEvent::RegisterProcess(pcb_med.clone())).await.unwrap();

    // Esperar a que se procese
    sleep(Duration::from_millis(200)).await;

    {
        let state = scheduler.read().await;
        // Debería estar "corriendo" (simulado)
        assert!(state.ready_queue.is_empty());
    }

    // 2. Inyectar tarea de prioridad alta (10)
    let pcb_high = PCB::new("HighPriority".to_string(), 10, "Prompt 2".to_string());
    tx.send(SchedulerEvent::RegisterProcess(pcb_high.clone())).await.unwrap();

    sleep(Duration::from_millis(100)).await;

    {
        let state = scheduler.read().await;
        // La tarea de prioridad 10 debería estar al frente de la cola de listos 
        // o haber disparado la lógica de preemption
        assert!(!state.ready_queue.is_empty());
        let top = state.ready_queue.peek().unwrap();
        assert_eq!(top.priority, 10);
    }
}
