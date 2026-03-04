# Changelog

## [1.0.2] - 2026-03-04
### Fixed
- **[ANK-112] Cold-Fix: Critical API & Mapping Mismatch**:
    - **Wasmtime v29 Compatibility**: Updated `preopened_dir` call in `PluginManager` to support the new 4-argument API, removing the deprecated `open_ambient_dir` requirement.
    - **Citadel Protocol Integrity**: Fixed missing `tenant_id` and `session_key` fields in the `PCB` to Protobuf conversion, ensuring full security context propagation during process teleportation.

## [1.0.1] - 2026-03-04
### Added
- **[ANK-111] Repository Security & Ignore Policy**:
    - **Strict `.gitignore`**: Implemented an exhaustive ignore policy blocking secrets (.env), credentials (*.key, *.pem), and local databases (*.sqlite).
    - **Metadata Isolation**: Automatically excluding AI agent internal states (/.agents/) and task tracking logs (/Tickets/) to maintain repository hygiene and privacy.
    - **Zero-Trust Defaults**: Pre-configured exclusions for logs, temporary file trees, and system-specific metadata (.DS_Store, Thumbs.db).

## [1.0.0] - 2026-03-03 "Immortal Core"
### Added
- **Final Architecture Integration**: Unified the Cognitive Scheduler, the cHAL, the VCM, the Scribe, the Swarm Discovery, the Wasm Sandbox, and the Citadel Protocol into a single cohesive production-ready Kernel.
- **Production SRE Pipeline**: Integrated the automated `deploy_debian.sh` script for full server lifecycle management.
- **Persistent Daemon**: Systemd service generation for high availability and zero-panic operations.
- **Tool Discovery & Prompt Injection**: Enhanced the IA's cognitive capabilities by inyecting dynamic "Skill Cards" and OS instructions into the inference stream.
- **Secure Cog-Net**: Host-level HTTP proxy with SSRF protection for Wasm plugins.

## [0.12.0] - 2026-03-03
### Added
- **[ANK-800] Aegis Standard Library (Wasm SDK & Core Plugins) (v0.10.0)**: Initial SDK.

## [0.12.0] - 2026-03-03
### Added
- **[ANK-900] Debian SRE Deployment Pipeline**:
    - **Automated Forge**: New `deploy_debian.sh` script for unattended installation of dependencies (OpenSSL, Protobuf, CMake).
    - **Plugin Distribution**: Automated compilation and deployment of Wasm binaries to the root `./plugins/` directory.
    - **SRE Hardening**: Script follows `set -euo pipefail` for fail-fast execution in production environments.
    - **Toolchain Management**: Automated setup of Rust and the `wasm32-wasi` target.

## [0.11.0] - 2026-03-03
### Added
- **[ANK-805] Tool Discovery & Prompt Injection**:
    - **Skill Cards**: Automatic generation of plugin usage guides (name, description, JSON format) for AI awareness.
    - **Master System Prompt**: Injection of OS-level directives (Cognitive ALU identity, Zero-Courtesy policy) in the HAL layer.
    - **Context Assembly**: Multi-layer prompt construction (System + Tools + User context) before dispatching to inference drivers.
    - **Plugin Metadata**: Enhanced plugin registration with parameter examples for improved Tool Use precision.

## [0.10.1] - 2026-03-03
### Added
- **[ANK-801] Cognitive Networking (std_net Plugin)**:
    - **SSRF Shield**: Implemented host-level DNS resolution and IP validation to prevent Server-Side Request Forgery.
    - **Kernel Interception**: Updated `PluginManager` to intercept `std_net` calls and perform safe fetches on behalf of the Wasm module.
    - **HTML Sanitization**: Pure Wasm machine-state logic for cleaning HTML (removing script/style tags) to optimize IA attention window.
    - **Safe Redirection**: Standardized host-to-guest data ingestion via virtual stdin pipes.

## [0.9.2] - 2026-03-02
### Added
- **[ANK-602] Secure Enclaves (SQLCipher)**:
    - **Encryption at Rest**: AES-256 mandatory encryption for tenant `memory.db` files via SQLCipher.
    - **Handshake Criptográfico**: The `TenantDB` struct enforces `PRAGMA key` using the tenant's `session_key`.
    - **Safe Storage**: New `kv_store` table for persistent, encrypted tenant-level state management.
    - **Portable Security**: Integrated `rusqlite` with `bundled-sqlcipher` for zero-dependence secure deployment.

## [0.9.1] - 2026-03-02
### Added
- **[ANK-601] Dynamic Jailing (Physical Isolation)**:
    - **VCM Sandboxing**: Path validation (`is_safe_path`) and context assembly now strictly enforce isolated tenant workspaces: `./users/{tenant_id}/workspace/`.
    - **Isolated Scribe Repos**: `ScribeManager` now manages independent Git repositories per tenant, ensuring data privacy and audit trails per user.
    - **Multi-Tenant Vector Memory**: `LanceSwapManager` computes tenant-specific paths (`./users/{tenant_id}/.aegis_swap/`) for isolated vector storage.
    - **Multi-Tenant Syscall Execution**: `SyscallExecutor` now routes `ReadFile` and `WriteFile` requests to the correct physically isolated tenant sandbox.

## [0.9.0] - 2026-03-02
### Added
- **[ANK-600] Citadel Protocol (gRPC Auth Interceptor)**:
    - **Security Perimeter**: Implemented a Tonic gRPC Interceptor (`auth_interceptor`) for mandatory multi-tenant validation.
    - **Zero-Knowledge Metadata**: Enforced headers `x-aegis-tenant-id` and `x-aegis-session-key` for all Kernel calls.
    - **Tenancy Propagation**: Updated `PCB` (Process Control Block) in `ank-core` to store `tenant_id` and `session_key` context.
    - **Sensitive Data Masking**: Implemented custom `Debug` for `PCB` to redact `session_key` from traces and logs.
    - **Protobuf Evolution**: Expanded `kernel.proto` with `optional tenant_id` fields in `TaskRequest` and `TaskSubscription` for client-side auditing.

## [0.8.1] - 2026-03-01
### Added
- **[ANK-402] Distributed Orchestration (Parallel DAG Execution)**:
    - **Parallel Task Emission**: Refactored `GraphManager::tick()` to emit multiple PCBs simultaneously for independent DAG nodes.
    - **Context Forwarding (Join/Gather)**: Implemented automated result injection where children nodes receive parent outputs via `inlined_context` (dependency_[id]).
    - **Swarm Orchestration**: Enabled the `CognitiveScheduler` to spread parallelized workload across the Neural Swarm.
    - **Sync Barrier**: Added `handle_result()` to the `GraphManager` to synchronize completed tasks and unlock dependent tiers of the DAG.
    - **Integration Test**: Validated a Diamond Graph structure (`A -> [B, C] -> D`) ensuring concurrent execution and correct state consolidation.

## [0.8.0] - 2026-03-01
### Added
- **[ANK-401] Process Migration (PCB Teleportation)**:
    - **Cognitive Teleportation**: Implemented `SwarmClient` for migrating full Process Control Blocks (PCBs) between nodes.
    - **Protobuf Extension**: Updated `kernel.proto` with the `TeleportProcess` RPC and `inlined_context` for lossless dependency migration.
    - **Distributed Scheduling**: Enhanced `CognitiveScheduler` to autonomously delegate high-complexity tasks to high-tier nodes.
    - **SRE Fallback (Resilience)**: Implemented automatic recovery where failed teleportations are re-queued locally, ensuring zero task loss.
    - **Context Inlining**: Added `inlined_context` support to PCB to bundle local files and dependencies before migration.

## [0.7.0] - 2026-02-28
### Added
- **[ANK-400] Neural Swarm Discovery (mDNS/Zeroconf)**:
    - **Zeroconf Foundation**: Integrated `mdns-sd` for automatic node discovery without static IPs.
    - **Self-Broadcasting**: Kernels now announce their `node_id`, `grpc_port`, and `hardware_tier` (Eco/Balanced/High-Perf).
    - **Active Registry**: Implemented `SwarmManager` with a thread-safe `RwLock` routing table for discovered nodes.
    - **Resilient Connectivity (SRE Audited)**: 
        - **Heartbeat Tolerance**: Implemented a 15-second grace period for unstable LAN connections via `NodeStatus::Suspect`.
        - **ID-Validation Guard**: Added session validation using timestamps to prevent race conditions during rapid re-connections.
        - **Delayed Removal**: Autonomous tasks to clean up unreachable nodes only after the grace period expires.
    - **Security**: Added filtering to ignore loopback/self-discovery events.

## [0.6.2] - 2026-02-28
### Added
- **[ANK-302] Chronos Daemon (Memory Consolidation)**:
    - **Background Asimilation**: Implemented an autonomous daemon that summarizes recent cognitive events during system idle periods.
    - **Idle State Detection**: Multi-layer monitor (ALU/Queue/Activity) to ensure background tasks never compete with user interaction.
    - **Resource Safety (Cooldown)**: Integrated a mandatory cooldown mechanism to prevent redundant task injection and DB saturation.
    - **Low-Priority Scheduling**: Automatic generation of Priority 1 PCBs for context compression and long-term semantic storage (L3).

## [0.6.1] - 2026-02-28
### Added
- **[ANK-301] Cognitive Syscalls & Plugin Binding**:
    - **Neural Syscall Executor**: Implemented a bridge between the IA and Kernel subsystems (Plugins, Scribe, VCM).
    - **Real-Time Stream Interceptor**: Added a rolling-buffer based monitor to detect syscall triggers (`[SYS_`) and halt inference mid-stream.
    - **Structured Grammar**: Defined a robust regex-based parser for `PluginCall`, `ReadFile`, and `WriteFile` commands.
    - **Contextual Injection**: Standardized the `[SYSTEM_RESULT: ...]` format for injecting external tool outputs back into the IA's attention window.

## [0.6.0] - 2026-02-28
### Added
- **[ANK-300] Wasm Plugin System (User Space)**:
    - **Performance Core**: Integrated `wasmtime` with JIT-accelerated modules and pre-cached `Linker` for sub-millisecond start times.
    - **Security (Ring 0 Level)**: Implemented strict sandboxing with no host filesystem/network access unless explicitly granted.
    - **Resource Management**: Integrated CPU "Fuel" consumption monitoring to prevent denial-of-service via infinite loops.
    - **Data Interchange**: Standardized JSON-based communication via WASI stdin/stdout virtual pipes for maximum safety and polyglot support.

## [0.5.5] - 2026-02-28
### Added
- **VCM Semantic Memory (L3)**: Integrated `LanceSwapManager` into the `VirtualContextManager`.
- **Vector Search Integration**: `assemble_context` now resolves `swap_refs` from the PCB using vector similarity search.
- **Cognitive Truncation**: Implemented safe truncation for L3 memory to respect the LLM's token limit without affecting the primary instruction.
- **[ANK-108] LanceDB Integration**: Core logic for the L3 Swap Manager using LanceDB and Apache Arrow v52.

## [0.5.4] - 2026-02-27

### Added
- **[ANK-107] The Scribe: Cognitive Traceability System**:
    - **Engine Core**: Integrated `git2` for automated version control of all AI-driven file writes.
    - **Transactional Integrity**: Implemented a concurrent-safe `ScribeManager` using `tokio::sync::Mutex` to prevent `index.lock` race conditions.
    - **Auditability**: Enforced mandatory `CommitMetadata` (task ID, versioning, impact) for all disk operations.
    - **Virtual Identity**: Established "ANK Scribe <ank@aegis.ia>" as the primary author for cognitive audits.
    - **Recovery**: Added `hard_reset` capability for mission-critical state restoration.

## [0.5.3] - 2026-02-28

### Added
- **[ANK-106] Virtual Context Manager (VCM) Implementation**:
    - **Cognitive Assembly**: Implemented the `VirtualContextManager` for deterministic prompt construction ([SYSTEM] + [L2] + [L1]).
    - **Security (Sandboxing)**: Integrated path traversal protection using component-depth validation for `file://` URIs.
    - **Memory Optimization**: 
        - Implemented "Check-Before-Read" logic using file metadata to prevent OOM on massive files.
        - Efficient string concatenation with pre-allocated buffers and minimal re-allocations.
    - **Resiliency**: Automated file omission with system notifications when exceeding the attention window's `token_limit`.
    - **Instrumentation**: Structured tracing for file load skips and VCM assembly events.

## [0.5.2] - 2026-02-28

### Added
- **[ANK-105] Native Driver Implementation (llama-cpp-2)**:
    - **Engine Core**: Integrated `llama.cpp` via `llama-cpp-2` for high-performance GGUF local inference.
    - **Async Streaming**: Implemented a non-blocking generation loop using `tokio::task::spawn_blocking` and asynchronous `mpsc` channels.
    - **Security Audit & FFI**: 
        - Manual `Send` + `Sync` implementation with a dedicated safety audit for `LlamaNativeDriver`.
        - Enforced strict memory drop order in `load_model` to prevent *Use-After-Free* and pointer invalidation.
    - **Resilient Decoding**: Integrated `encoding_rs` for robust UTF-8 token-to-string conversion, correctly handling multibyte characters split across tokens.
    - **Optimization**: Configurable GPU offloading (`n_gpu_layers`) and context management.

## [0.5.1] - 2026-02-28

### Added
- **[ANK-104] Cognitive HAL & Inference Traits Implementation**:
    - **Abstraction Layer**: Defined the `InferenceDriver` trait using `async-trait` for hardware-agnostic execution.
    - **Smart Routing**: Implemented `route_and_execute` in `CognitiveHAL` with policy-based selection (`LocalOnly`, `CloudOnly`, `HybridSmart`).
    - **Security & Performance**: Refactored lock management to release `SharedPCB` read guards before inference, preventing deadlocks.
    - **Streaming**: Native support for asynchronous token streaming via `Pin<Box<dyn Stream>>`.
    - **Error Handling**: Introduced structured `SystemError` and `ExecutionError` using `thiserror`.
    - **Validation**: Added `DummyDriver` and unit tests for priority-based and complexity-based routing.

## [0.5.0] - 2026-02-28

### Changed
- **[ANK-102] Cognitive Scheduler Audit & Logic Implementation**:
    - **Engine Core**: Implemented the asynchronous `start()` loop using `tokio::select!` for deterministic task management.
    - **Priority Dispatching**: Refined the `reconcile()` logic to enforce a strict Max-Heap priority policy (Priority 10 preempts/dispatches first).
    - **Zero-Panic Policy**: All error paths refactored to use `anyhow::Result` and proper context propagation.
    - **Instrumentation**: Added comprehensive `tracing::instrument` and structured logging for PID state transitions.
    - **Stability Fix**: Patched `chrono` dependency to `0.4.38` via workspace pinning to resolve a breaking conflict with `arrow-arith`.
    - **Reliability**: Added unit tests verifying priority-based queuing and First-Come First-Served (FCFS) tie-breaking desing.

## [0.4.0] - 2026-02-27

### Changed
- **[ANK-100 & ANK-101] Audit & Professional Refinement**:
    - **Workspace Architecture**: Standardized root `Cargo.toml` with centralized `[workspace.dependencies]`.
    - **Shared Crates**: Migrated `tokio`, `tonic`, `prost`, `serde`, `anyhow`, and `llama-cpp-2` to workspace-level versioning.
    - **Protobuf Contract**: Refactored `kernel.proto` for high-fidelity cognitive orchestration.
        - Detailed `PCB` (Process Control Block) with state transitions, quantum tracking, and parent-child hierarchy.
        - Structured `Syscall` message for deterministic tool invocation.
        - Enriched `SystemStatus` with VRAM telemetry and worker load metrics.
    - **Build Pipeline**: Enhanced `ank-proto` with automated `Serde` derivation only for internal types to prevent compilation edge cases.
    - **Crate Standardization**: Cleaned up `ank-core` and `ank-server` to inherit dependencies from the workspace.

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
