# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-02-27

### Added
- **[ANK-103]** ANK-Bridge (gRPC Server) Implementation:
    - High-performance gRPC server using `tonic`.
    - `SubmitTask` endpoint for mission ingestion.
    - `WatchTask` for server-side streaming of cognitive events.
    - `GetSystemStatus` for hardware and model telemetry.
    - Internal Event Broker for routing PID-specific events to gRPC clients.
    - Protobuf contract updated with `TaskSubscription` and proper streaming types.
- **Integration Tests**:
    - `tests/test_bridge.rs` for verifying gRPC service logic and scheduler integration.

## [0.2.0] - 2026-02-27

### Added
- **[ANK-102]** Cognitive Scheduler Implementation:
    - High-performance `BinaryHeap` priority queue for task management.
    - Async execution loop using `tokio::select!` for reactive event handling.
    - Preemption logic for Priority 10 (Critical) tasks.
    - Process Control Block (PCB) state machine implementation (NEW -> READY -> RUNNING).
    - Thread-safe state management using `Arc<RwLock<Scheduler>>`.
- **Integration Tests**:
    - `tests/test_scheduler.rs` verifying priority dispatching and concurrent execution.

## [0.1.0] - 2026-02-27

### Added
- **[ANK-100]** Initial Cargo Workspace structure:
    - `ank-core`: Core engine logic (Scheduler, cHAL, VCM).
    - `ank-proto`: Protobuf compilation and Rust gRPC bindings.
    - `ank-server`: Main daemon binary.
- **[ANK-101]** gRPC Contract Specification:
    - `proto/kernel.proto` defining the communication layer.
    - PCB (Process Control Block) structure for cognitive state management.
    - Task submission and event streaming definitions.

### Technical Details
- Workspace dependencies configured for `tokio`, `tonic`, `prost`, `serde`, and `anyhow`.
- `build.rs` automation for `ank-proto` to compile `.proto` files at build time.
