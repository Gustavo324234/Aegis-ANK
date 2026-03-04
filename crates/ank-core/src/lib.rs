pub mod chal;
pub mod chronos;
pub mod dag;
pub mod pcb;
pub mod plugins;
pub mod scheduler;
pub mod scribe;
pub mod swarm; // Added pub mod swarm;
pub mod syscalls;
pub mod vcm;
pub mod enclave;

// Re-exportar para fácil acceso
pub use chal::{CognitiveHAL, InferenceDriver, SystemError};
pub use chronos::ChronosDaemon;
pub use dag::{GraphManager, DagNode, DagNodeStatus, ExecutionGraph, NodeResult};
pub use pcb::{ProcessState, PCB};
pub use scheduler::{CognitiveScheduler, SchedulerEvent, SharedScheduler};
pub use swarm::SwarmManager;
pub use syscalls::{parse_syscall, Syscall}; // Added re-export for SwarmManager
pub use enclave::TenantDB;
