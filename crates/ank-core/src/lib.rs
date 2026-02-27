pub mod pcb;
pub mod scheduler;

// Re-exportar para fácil acceso
pub use pcb::{PCB, ProcessState};
pub use scheduler::{Scheduler, SchedulerEvent};
