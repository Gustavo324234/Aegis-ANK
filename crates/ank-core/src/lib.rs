pub mod chal;
pub mod chronos;
pub mod dag;
pub mod enclave;
pub mod pcb;
pub mod plugins;
pub mod scheduler;
pub mod scribe;
pub mod swarm; // Added pub mod swarm;
pub mod syscalls;
pub mod vcm;

// Re-exportar para fácil acceso
pub use chal::{CognitiveHAL, InferenceDriver, SystemError};
pub use chronos::ChronosDaemon;
pub use dag::{DagNode, DagNodeStatus, ExecutionGraph, GraphManager, NodeResult};
pub use enclave::TenantDB;
pub use pcb::{ProcessState, PCB};
pub use scheduler::persistence::{SQLCipherPersistor, StatePersistor};
pub use scheduler::{CognitiveScheduler, SchedulerEvent, SharedScheduler};
pub use swarm::SwarmManager;
pub use syscalls::{parse_syscall, Syscall}; // Added re-export for SwarmManager
