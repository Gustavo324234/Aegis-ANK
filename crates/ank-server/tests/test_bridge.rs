use ank_core::SchedulerEvent;
use ank_proto::v1::kernel_service_server::KernelService;
use ank_proto::v1::{Priority as ProtoPriority, TaskRequest};
use ank_server::server::AnkRpcServer;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tonic::Request;

#[tokio::test]
async fn test_submit_task_logic() -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::channel(10);
    let broker = Arc::new(RwLock::new(HashMap::new()));
    let server = AnkRpcServer::new(tx, broker);

    let request = Request::new(TaskRequest {
        prompt: "Test prompt".to_string(),
        priority: ProtoPriority::Normal as i32,
        policy: None,
        initial_context: None,
    });

    let response = server
        .submit_task(request)
        .await
        .context("Failed to submit task")?;
    let inner = response.into_inner();

    assert!(inner.accepted);
    assert!(inner.pid.starts_with("proc_"));

    // Verificar que el scheduler recibió el evento
    let event = rx.try_recv().context("No event received")?;
    if let SchedulerEvent::ScheduleTask(pcb) = event {
        assert_eq!(pcb.pid, inner.pid);
        assert_eq!(pcb.priority, 5);
        assert_eq!(pcb.process_name, "Remote Task");
    } else {
        anyhow::bail!("Wrong event type");
    }
    Ok(())
}
