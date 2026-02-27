use ank_core::SchedulerEvent;
use ank_server::server::AnkRpcServer;
use ank_proto::v1::kernel_service_server::KernelService;
use ank_proto::v1::{TaskRequest, Priority as ProtoPriority};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tonic::Request;

#[tokio::test]
async fn test_submit_task_logic() {
    let (tx, mut rx) = mpsc::channel(10);
    let broker = Arc::new(RwLock::new(HashMap::new()));
    let server = AnkRpcServer::new(tx, broker);

    let request = Request::new(TaskRequest {
        prompt: "Test prompt".to_string(),
        priority: ProtoPriority::Medium as i32,
        model_pref: None,
    });

    let response = server.submit_task(request).await.unwrap();
    let inner = response.into_inner();

    assert!(inner.accepted);
    assert!(inner.pid.starts_with("proc_"));

    // Verificar que el scheduler recibió el evento
    let event = rx.try_recv().unwrap();
    if let SchedulerEvent::RegisterProcess(pcb) = event {
        assert_eq!(pcb.pid, inner.pid);
        assert_eq!(pcb.priority, 5);
        assert_eq!(pcb.memory_pointers.l1_instruction, "Test prompt");
    } else {
        panic!("Wrong event type");
    }
}
