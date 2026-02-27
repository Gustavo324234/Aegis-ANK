use crate::pcb::{PCB, ProcessState};
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

pub enum SchedulerEvent {
    RegisterProcess(PCB),
    SyscallCompleted { pid: String, result: String },
    ProcessTerminated { pid: String, success: bool },
}

pub struct Scheduler {
    ready_queue: BinaryHeap<PCB>,
    waiting_queue: HashMap<String, PCB>,
    current_running: Option<PCB>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            ready_queue: BinaryHeap::new(),
            waiting_queue: HashMap::new(),
            current_running: None,
        }
    }

    pub async fn run(
        scheduler_state: Arc<RwLock<Self>>,
        mut event_rx: mpsc::Receiver<SchedulerEvent>,
    ) {
        info!("Cognitive Scheduler started.");

        loop {
            tokio::select! {
                // Escuchar nuevos eventos (Nuevos procesos, syscalls terminadas)
                Some(event) = event_rx.recv() => {
                    let mut state = scheduler_state.write().await;
                    match event {
                        SchedulerEvent::RegisterProcess(mut pcb) => {
                            info!("New process registered: {} (Priority: {})", pcb.pid, pcb.priority);
                            pcb.state = ProcessState::Ready;
                            
                            // Lógica de Preemption simplificada: 
                            // Si llega algo de prioridad 10 y lo que corre es menor, marcar para interrupción
                            if pcb.priority >= 10 {
                                if let Some(ref running) = state.current_running {
                                    if running.priority < 10 {
                                        warn!("High priority preemption triggered by {}", pcb.pid);
                                        // Aquí se enviaría una señal al cHAL para detener la inferencia actual
                                    }
                                }
                            }
                            state.ready_queue.push(pcb);
                        }
                        SchedulerEvent::SyscallCompleted { pid, result } => {
                            if let Some(mut pcb) = state.waiting_queue.remove(&pid) {
                                info!("Syscall completed for {}. Moving back to Ready.", pid);
                                pcb.registers.temp_vars.insert("last_syscall_res".to_string(), result);
                                pcb.state = ProcessState::Ready;
                                state.ready_queue.push(pcb);
                            }
                        }
                        SchedulerEvent::ProcessTerminated { pid, success } => {
                            info!("Process {} terminated (Success: {})", pid, success);
                            if let Some(ref running) = state.current_running {
                                if running.pid == pid {
                                    state.current_running = None;
                                }
                            }
                        }
                    }
                }

                // Si no hay eventos, y hay procesos listos, y la "CPU" está libre, despachar
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    let mut state = scheduler_state.write().await;
                    if state.current_running.is_none() && !state.ready_queue.is_empty() {
                        if let Some(mut pcb) = state.ready_queue.pop() {
                            info!("Dispatching process: {} (Priority: {})", pcb.pid, pcb.priority);
                            pcb.state = ProcessState::Running;
                            state.current_running = Some(pcb);
                            
                            // TODO: Llamar al dispatch_to_cHAL(pcb) en Q1-Q2 2026
                        }
                    }
                }
            }
        }
    }
}
