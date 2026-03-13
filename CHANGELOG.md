# Aegis Neural Kernel (ANK) - Changelog

## [2.1.0] - Unreleased

### Added
- **[ANK-901] SRE Firewall - CI Pull Request Guard:**
  - Implementación de `.github/workflows/pr_check.yml` para la auditoría automática de integridad en el repositorio del Kernel.
  - Configuración de "The Forge CI" con caché optimizada mediante `rust-cache` para reducir tiempos de build.
  - Aplicación de la política Zero-Panic a nivel de compilador denegando `unwrap_used` y `expect_used` mediante Clippy.
  - Automatización del chequeo de formato (`cargo fmt`) y ejecución de suites de test en cada PR dirigido a `main`.

- **[ANK-3003] MCP Cognitive Binding (Tool Discovery & Execution):**
  - Implementación de `McpToolRegistry` para la gestión dinámica de herramientas externas vía Model Context Protocol.
  - Interrogación automática de capacidades mediante `tools/list` al iniciar sesiones MCP.
  - Mapeo cognitivo de esquemas JSON a System Prompt nativo para el LLM.
  - Implementación de la Syscall `[SYS_MCP_EXEC(tool_name, args)]` con validación estricta de argumentos.
  - Integración en `CognitiveHAL` para inyección de prompts y enrutamiento de despacho.

- **[ANK-2301] VCM Tensor-Compressor (INT8 Quantization):**
  - Implemented symmetric Min/Max scaling for vector compression to reduce L3 Swap memory footprint by 75%.
  - Added `quantize_f32_to_i8` and `dequantize_i8_to_f32` with Zero-Panic division guards.
  - Refactored `MemoryFragment` to support optional INT8 storage, offloading the original f32 vectors after compression.
  - Validated mathematical precision with dedicated unit tests for quantization roundtrips.
- **[ANK-2303] CCSP & Context Slicing (VCM Intelligence):**
  - Refactored `assemble_context` to implement intelligent token budgeting and hierarchical truncation.
  - Implemented S-DAG priority: `inlined_context` (DAG Dependencies) is now immutable and prioritized over history and swap.
  - Added smart truncation for `L2_CONTEXT`: history is sliced from the oldest message (tail-trimming) to preserve recent context while staying under `token_limit`.
  - Integrated `.env` aware budgeting: dynamically adjusts limits for `CloudOnly` models using `CLOUD_MAX_TOKENS`.
  - Optimized memory allocation using `String::with_capacity` based on token heuristics to minimize fragmentation.

### Security / Hardening
- **[ANK-2410] Citadel Identity Hardening (Zero-Leakage):**
  - Implementación de ofuscación criptográfica para el `tenant_id` utilizando HMAC-SHA256 y Base64 URL-Safe.
  - Modificado el `CitadelInterceptor` para inyectar el struct `SafeIdentity` en las extensiones de la request, separando el ID privado (interno) del ID público (telemetría).
  - Sanitización proactiva de logs y errores gRPC: se ha reemplazado el uso del ID real por su alias en todos los eventos de `tracing` y mensajes de `tonic::Status`.
  - Refactorizado el `PCB` (Process Control Block) para incluir `public_id` y redactar el `tenant_id` original en sus implementaciones de `Debug`.
  - Garantizado el flujo Zero-Panic: el sistema rechaza conexiones (`UNAUTHENTICATED`) si la `AEGIS_ROOT_KEY` no está configurada, evitando fallos catastróficos.
- **[ANK-2411] Plugin Signature Hardening & Trap Classification (Ring 0):**
  - Implementación de verificación obligatoria de firmas Ed25519 para todos los plugins `.wasm` mediante la nueva utilidad `PluginSigner`.
  - Refactorización del `PluginManager` para clasificar traps de Wasmtime en `SecurityViolation`, `LogicError` y `ResourceExhaustion`.
  - Introducción de la política "Tainted": ante una violación de seguridad (`MemoryOutOfBounds`), el plugin es marcado automáticamente como `TAINTED` en la base de datos `TenantDB`, bloqueando ejecuciones futuras.
  - Hardening del cargador de plugins con política Zero-Panic, propagando errores de firma mediante `anyhow` y registrando alertas SRE sin interrumpir el Kernel.
- **[ANK-2412] PersistenceManager & Atomic State Reconciliation (SQLCipher):**
  - Creación del `PersistenceManager` encapsulando transacciones de `SQLCipher` mediante `rusqlite` para la persistencia del estado del Scheduler.
  - Modificación del `CognitiveScheduler` para implementar persistencia atómica: cada transición de estado de un PCB se guarda en disco *antes* de ser procesada en RAM.
  - Implementación de Inyección de Dependencias (`StatePersistor`) permitiendo el uso de mocks en pruebas unitarias y SQLite cifrado en producción.
  - Adición de un handler de señales `SIGTERM/Ctrl+C` en el binario principal para asegurar un `flush` definitivo de la base de datos antes del cierre del sistema.
- **[ANK-2413] S-DAG Graph Compiler & Deterministic Topological Validation:**
  - Implementación del `GraphCompiler` con validación topográfica mediante DFS para la detección eficiente de ciclos en grafos S-DAG.
  - Validación de "Dangling Dependencies" para garantizar que cada ID de dependencia refiera a un nodo existente en el grafo.
  - Creación del `GraphIntegrator` como middleware de seguridad entre el orquestador cognitivo y el Scheduler.
  - Política SRE Zero-Panic: integración de un mecanismo de fallback que genera un grafo monolítico de emergencia ante fallos de compilación, asegurando el 100% de disponibilidad del sistema.
  - Ampliación del bus de eventos del Kernel con el evento `RegisterGraph` para el registro seguro de grafos validados.

### Epic 15 / Integration (Universal Integration - MCP)
- **[ANK-2414] McpTransport Trait & SSE Client Implementation (v1.3.0):**
  - Creación del crate `ank-mcp` para la comunicación con herramientas externas vía Model Context Protocol.
  - Definición del trait `McpTransport` para el intercambio asíncrono de mensajes JSON-RPC 2.0.
  - Implementación de `SseTransport` sobre `reqwest`, permitiendo la conexión a Sidecars MCP mediante streams SSE para recepción y HTTP POST para envío.
  - Robustez Industrial: Implementación de un parser SSE resiliente que maneja fragmentación de red y timeouts explícitos de grado SRE.
  - TDD Garantizado: Inclusión de tests de integración con un servidor mock (`warp`) validando el flujo bidireccional de mensajes en el Ring 0.
- **[ANK-2415] StdIO Transport & Subprocess Jailing:**
  - Implementación de `StdioTransport` para la ejecución de servidores MCP locales mediante subprocesos nativos.
  - **Zero-Trust Isolation**: Integración obligatoria de `env_clear()` para evitar la herencia de secretos del Kernel y `current_dir()` para el confinamiento físico en el workspace del tenant.
  - **I/O Asíncrono**: Uso de `BufReader` y streams de `Lines` para el parsing eficiente de mensajes JSON-RPC 2.0 delimitados por `\n`.
  - **Lifecycle Management (Zombie Killer)**: Implementación manual del trait `Drop` para garantizar la terminación inmediata (`start_kill`) del proceso hijo, evitando procesos huérfanos en el host.
  - **Zero-Panic SRE**: Propagación elegante de errores de "Broken Pipe" y fallos de spawn mediante `anyhow`, asegurando la estabilidad del Ring 0 ante fallos del servidor MCP.
- **[ANK-3002] Async Multiplexer & Client Session (The Actor):**
  - Implementación de `McpClientSession` siguiendo el Patrón Actor para la multiplexación asíncrona de mensajes JSON-RPC.
  - **Multiplexación de Promesas**: Uso de un diccionario de promesas con canales `oneshot` de Tokio indexados por UUIDs para correlacionar peticiones y respuestas concurrentes.
  - **Resiliencia Hardened**: Implementación obligatoria de timeouts de 30 segundos en todas las llamadas `call` para evitar bloqueos del S-DAG.
  - **Notificación de Cierre**: Mecanismo de limpieza que notifica proactivamente a todas las peticiones pendientes (`ConnectionClosed`) si el transporte subyecente colapsa.
  - **Zero-Panic Architectural Design**: Gestión segura de canales y locks de concurrencia (`Arc<Mutex<HashMap>>`) evitando deadlocks y garantizando robustez industrial.

### Refactored
- **[ANK-2202] Scatter-Gather Scheduler & S-DAG Synchronization:**
  - Integrated `GraphManager` into the `CognitiveScheduler` internal state using an `Arc<RwLock<GraphManager>>` for zero-penalty concurrent access.
  - Implemented the `ProcessCompleted` scheduler event to accurately intercept finished processes and notify the `GraphManager` via `handle_result()`.
  - Added deterministic anti-deadlock safeguards performing granular lock acquisitions and fast-drops when advancing the S-DAG via `tick()`.
  - Validated native Context Forwarding, confirming that child PCBs automatically inherit memory pointers (`dependency_[id]`) from completed parallel dependencies before entering the `ready_queue`.
- **[ANK-2203] Hybrid Sub-Agent Routing (CognitiveHAL):**
  - Refactored `route_and_execute` to securely manage heterogeneous driver delegation using `ModelPreference`.
  - Implemented strict Microkernel compliance leveraging `#[cfg(feature = "local_llm")]` to return elegant `HardwareFailure` messages if local execution is requested on a thin build.
  - Enhanced `HybridSmart` heuristic to dynamically fallback to `cloud-driver` if the local driver is missing or the task's complexity exceeds cognitive thresholds.
  - Hardened concurrent bounds by explicitly scoping the `shared_pcb.read().await` lock, releasing it *before* the inference stream invocation.
- **[ANK-2201] Grammar-Based S-DAG Planner & Hybrid Routing:**
  - Expanded `DagNode` struct to include `required_model: ModelPreference`.
  - Upgraded the DAG `GraphManager` to propagate `required_model` into the resulting `PCB`'s `model_pref` feature during parallel execution mappings via `tick()`.
  - Created a built-in `generate_dag_from_prompt` method with a zero-panic SRE fallback mechanism. If the system fails to parse an S-DAG natively using `serde_json`, it constructs a deterministic monolithic single-node execution graph fallback with `HybridSmart` to ensure 100% stability.
- **[ANK-2003] Siren V2 - Pluggable Audio Architecture:**
  - Rediseñado el protocolo Siren para instanciar automáticamente el `CloudVoiceDriver` cuando se compila sin `local_voice`.
  - Transcripción asíncrona mediante `/v1/audio/transcriptions` utilizando `reqwest::multipart`.
  - Síntesis de voz dinámica apuntando a `/v1/audio/speech`, manteniendo TTS concurrente y delegando bloqueos a la red.
  - Implementada lógica de VAD diferida consumiendo la señal de control "VAD_END_SIGNAL" desde la Shell, acumulando los chunks eficientemente cuando el driver C++ no está disponible.
- **[ANK-2002] Modularización de Dependencias Pesadas (Cargo Features):**
  - Transformado el Kernel en un modelo "Microkernel" activando bindings pesados de C++ (`llama-cpp-2`, `whisper-rs`, `webrtc-vad`) exclusivamente a través de los *features* `local_llm`, `local_voice` y `full_local`.
  - Aplicados mecanismos *Graceful Degradation* (Zero-Panic) para retornar estatus `unimplemented` en servicios gRPC nativos si los módulos locales no están incluidos en el *build*.

### Fixed
- **[ANK-903] Dockerfile Protoc Hotfix:**
  - Inyección de `protobuf-compiler` en la etapa `builder` para solucionar fallos de compilación de `ank-proto`.
  - Optimización de la capa mediante limpieza de caché de `apt`.

- **[ANK-904] CI/CD Protoc Injection (SRE Firewall):**
  - Actualización de `.github/workflows/pr_check.yml` para instalar `protobuf-compiler` en el runner de GitHub Actions.
  - Garantizada la integridad del pipeline de auditoría automática ("The Forge").

## [1.5.1] - Unreleased

### Added
- **[ANK-133] Dynamic Engine Configuration:**
  - Added `ConfigureEngine` gRPC inside `kernel.proto`.
  - Secure storage of cloud inference credentials per Tenant using SQLCipher `TenantDB`.
  - Dynamic `CognitiveHAL` reconfiguration at runtime without needing a Rust binary restart.

## [1.5.0] - "Siren's Call" (GOLD MASTER RELEASE)
### Added
- **[ANK-132] Universal Cloud Driver (OpenAI-Compatible)**:
    - **Cognitive Elasticity**: Implemented `CloudProxyDriver` utilizing `reqwest` async streams for external API connections.
    - **SSE Resiliency**: Dynamic parsing of `Server-Sent Events` data blocks without `.unwrap()`, extracting text seamlessly with a line-buffer state machine.
    - **Auto-Discovery**: `CognitiveHAL` now automatically instantiates the driver if `AEGIS_CLOUD_API_URL` and `AEGIS_CLOUD_API_KEY` are detected.
    - **Error Mapping**: Robust resolution of HTML status errors into `SystemError::HardwareFailure`.

## [1.5.1] - 2026-03-07 (Security Patches)
### Security
- **[SEC-005] Insecure Hashing**: Reemplazado `sha2` por `argon2` para el hashing de contraseñas de Master Admin y Tenants en Rust, utilizando sales aleatorias (`Argon2id`).
- **[SEC-006] Session Key Leak Protection**: Eliminado el campo `session_key` del contrato `PCB` en `kernel.proto` para evitar su transmisión insegura. Modificado el mapeo de `TeleportProcess` en el Swarm Client y Server para no enviar telemetría confidencial a otros nodos.
- **[SEC-009] Tenant Password Persistence**: Actualizado el esquema de base de datos en `MasterEnclave` para almacenar hashes persistentes (`password_hash`) de los Tenants. Añadida la lógica independiente `authenticate_tenant` para validaciones SRE estructuradas en el servidor gRPC.

### Performance & Stability Patches (Batch 2)
- 🚀 **[SRE-004/013] Cognitive & Broker GC (Memory Leak Plug)**: Integrado Garbage Collector paralelo para purga de procesos `Completed`/`Failed` > 5min en el scheduler y purga de descriptores colgantes en el sistema de subscripción de la capa gRPC.
- 🛡️ **[SEC-010] Syscall Jailing**: Inyectado de validación `is_safe_path` mediante el Virtual Context Manager en ejecución de rutinas sensibles de disco `[READ_FILE]` y `[WRITE_FILE]` para mitigar un posible LFI local.
- ⚡ **[STB-017/020] Fail-Fast Boot**: Eliminada dependencia en `default_root_key` insegura en MasterEnclave; requerido explícito de la variable de ambiente estricta `AEGIS_ROOT_KEY`. Mantenibilidad a largo plazo vía strict `.expect("FATAL")` para las Syscalls de tipo Regex.

## [1.5.0] - 2026-03-06
### Added
- **[ANK-125] Siren Protocol TTS (Voz Zero-Blocking)**:
    - **Contrato Siren Actualizado**: Modificación de `siren.proto` para añadir `tts_audio_chunk` y `sample_rate` al `SirenEvent`, unificando telemetría y voice streaming.
    - **Acumulador de Oraciones Matemático**: Implementación de `SentenceAccumulator` que bufferiza los tokens del LLM a máxima velocidad y aplica una partición heurística por oraciones (`.`, `?`, `!`, `\n`) antes de pasarlos a síntesis.
    - **Pipeline TTS Asíncrono**: Creación del Worker TTS en `spawn_blocking` con un canal concurrente MPSC que escucha las oraciones recolectadas y emite un flujo ininterrumpido a `SirenStream`, separando al LLM del costo de inferencia de audio.
    - **Intercepción y Multiplexión de UI**: Refactorizado de la inyección de eventos para que el `SentenceAccumulator` escuche al LLM de fondo en paralelo con el usuario leyendo visualmente desde The Orb sin bloqueos cruzados.

## [1.4.0] - 2026-03-06
### Added
- **[ANK-131] Aegis Admin CLI (Terminal Interface)**:
    - **Native Binary**: Creación del crate `ank-cli` utilizando `clap` v4 para la gestión del Kernel en modo Headless (Terminal). Integrado al Workspace global.
    - **Citadel Protocol**: Cliente gRPC implementado con interceptor que captura automáticamente `AEGIS_TENANT_ID` y `AEGIS_SESSION_KEY` del entorno operativo, integrándose con la capa Zero-Trust del Kernel.
    - **Procesos en Tiempo Real**: Refactor del `ListProcesses` gRPC endpoint en `ank-server` implementando el modelo Actor-Pattern (mpsc/oneshot channel) para interrogar atómicamente al `CognitiveScheduler` sin desarmar su inner loop, exponiendo de forma read-only el mapa de memoria de los PCBs activos.
    - **Streaming Cognitivo (`aegis run`)**: El subcomando orquesta eficientemente una llamada `SubmitTask` para capturar el PID seguido de una suscripción inmediata a `WatchTask`, emitiendo asincrónicamente el stream de tokens `Thought` a `stdout`.
    - **Zero-Panic SRE**: Hook asíncrono para interrupciones de sistema (`Ctrl+C`) capturado por `tokio::signal` abortando elegantemente el stream transitorio (`std::process::exit(0)`) antes de inducir *panics* por bindings internos (pipe rotos, abortos gRPC forzados).
- **[ANK-802] Zero-Downtime Wasm Hot-Reloading**:
    - **Demons Watcher**: Implementación asíncrona de un daemon sobre la ruta `./plugins` utilizando el crate `notify` y `notify-debouncer-mini`.
    - **Atomic Hot-Swap Lock**: Refactor completo del `PluginManager` para soportar bloqueos de lectura/escritura concurrentes a través de `tokio::sync::RwLock`.
    - **Zero-Panic Validation**: El Kernel ahora compila dinámicamente el `.wasm` *antes* de adquirir el `write().await` sobre el mapa de plugins, evitando la ralentización de inferencia general y atrapando builds corruptas de la comunidad sin sacrificar la versión actual cargada en RAM.
    - **Auto-Discovery**: La recarga extrae metadatos y herramientas de la nueva funcionalidad automáticamente bajo el namespace de Tenant SRE `system`.

## [1.3.0] - 2026-03-06
### Added
- **[ANK-130] Aegis Wasm SDK (Zero-Boilerplate Wrapper)**:
    - **Crate Native**: Creación del crate `aegis-sdk` en el workspace, abstrayendo boilerplate para futuros plugins Rust-Wasm.
    - **Zero-Boilerplate**: Implementación de la función central `run_plugin` para abstraer Stdin/Stdout de forma segura y propagar errores a un `PluginResponse` estructurado sin provocar Panics.
    - **Autodiscovery System**: Eliminación de descripciones hardcodeadas en el Kernel. Los plugins ahora contestan autonómicamente a `{"action": "get_metadata"}` proporcionando sus tarjetas de habilidad.
    - **SRE Hardening**: Soporte de intercepción cognitiva y bypass para metadatos (ej: `std_net`), permitiendo que funcionen en simetría con la extracción de datos segura provista por el Kernel Ring 0.
    - **Refactor Core Plugins**: Migración de `std_fs`, `std_sys`, y `std_net` reduciendo masivamente sus líneas de código delegando todo el control lógico al SDK centralizado.

## [1.2.2] - 2026-03-06
### Added
- **[ANK-124] Local Speech-to-Text (Whisper Offloading)**:
    - **Inferencia ML Local**: Integración de `whisper-rs` (Bindings whisper.cpp) para transcripción de audio en Ring 0 sin dependencias de nube.
    - **Zero-Blocking Architecture**: Implementación de `tokio::task::spawn_blocking` para aislar la carga pesada de inferencia del runtime asíncrono, garantizando la estabilidad de los streams.
    - **Filtro de Alucinaciones**: Sistema de limpieza heurística para descartar transcripciones de ruido o silencios generados erróneamente por el motor ML.
    - **Auto-Task Injection**: Los comandos de voz transcritos se inyectan automáticamente en el `CognitiveScheduler` como tareas de prioridad Crítica (10).
    - **Telemetría STT**: Nuevos eventos `STT_START`, `STT_DONE` y `STT_ERROR` para reportar el progreso de transcripción en tiempo real a la Shell.

## [1.2.1] - 2026-03-06
### Added
- **[ANK-123] Native Voice Activity Detection (VAD)**:
    - **Algoritmo WebRTC**: Integración del crate `webrtc-vad` para la detección de voz en tiempo real con alta agresividad.
    - **Accumulator Buffer**: Implementación de un buffer de sincronización matemática que garantiza frames exactos de 20ms (640 bytes @ 16kHz) antes de cada análisis VAD.
    - **Máquina de Estados de Voz**: Sistema de transición `SILENCE` <-> `SPEECH` con tolerancia de 600ms (30 frames) para evitar cortes prematuros durante pausas naturales de respiración.
    - **Eventos de Control**: Emisión automática de `VAD_START` y `VAD_END` vía gRPC para sincronizar el estado visual de la Shell (The Orb).

## [1.2.0] - 2026-03-05
### Added
- **[ANK-122] Protocolo Siren (gRPC Stream)**:
    - **Contrato Bidireccional**: Se definió `SirenService` en `siren.proto` con streaming concurrente de `AudioChunk` y control de telemetría para voz interactiva.
    - **Gestión Asíncrona (SRE)**: Implementación segura mediante `tokio::spawn` aislando el hilo gRPC principal del procesamiento de audio.
    - **Backpressure Nativo**: Uso de un canal `tokio::sync::mpsc` de capacidad limitada (200 chunks). Si el consumo se retrasa, el kernel devuelve `RESOURCE_EXHAUSTED` forzando al cliente (Shell) a ralentizar el envío, previniendo *Out Of Memory* (OOM).
    - **Test de Resiliencia**: Se incluyó un test integrado simulando Jitter de red para validar el rechazo de sobrecarga de manera predecible.

## [1.1.2] - 2026-03-05
### Added
- **[ANK-115] Workflow: GitHub Action Code Bundler**:
    - **Automated Bundling**: Created a GitHub Action workflow (`bundle_code.yml`) that traverses the repository and concatenates all relevant code (`.rs`, `.toml`, `.md`, `.proto`) into a single text file.
    - **Artifact Generation**: The workflow generates an artifact named `AegisAnkCode` containing the `AegisAnkCode.txt` bundle, optimized for LLM context ingestion.
    - **Filter Logic**: Implemented exclusion of `target/`, `.git/`, `.agents/`, and other non-source directories to maintain a clean and relevant context.
    - **Manual Trigger**: Added `workflow_dispatch` support to allow manual execution from the GitHub Actions tab.

## [1.1.1] - 2026-03-04
### Fixed
- **[ANK-114] Identity Initialization & System Status Robustness**:
    - **Robust Init Detection**: Refactored `MasterEnclave::is_initialized()` to verify `master_admin` table existence before querying, preventing false positives or errors on empty databases.
    - **Explicit Status Reporting**: Updated `GetSystemStatus` in `AnkRpcServer` to return `SystemState::StateInitializing` (0) when the database is missing or uninitialized, enabling the Shell's setup flow.
    - **Non-Blocking Interceptor**: Verified and documented that `auth_interceptor` permits unauthenticated `GetSystemStatus` calls, essential for the initial admin creation bootstrap.
    - **Strict Admin Validation**: Enhanced `authenticate_master` to validate both username and password hash, ensuring `CreateTenant` and `ResetTenantPassword` are strictly reserved for the Master Admin.

## [1.1.0] - 2026-03-04
### Added
- **[ANK-603] Identity & Tenant Management**:
    - **Master Admin Enclave**: Implemented `admin.db` with SQLCipher persistence and SHA-256 password hashing for robust Super Admin management.
    - **Tenant Orchestration**: Added gRPC endpoints `InitializeMasterAdmin`, `CreateTenant`, and `ResetTenantPassword` in `KernelService`.
    - **Dynamic Port Assignment**: Automated network port allocation for new UI/BFF tenants, mapped directly in the internal SQLite store.
    - **Kernel System State**: Added `SystemState` enum to telemetry (`GetSystemStatus`) to represent `INITIALIZING` or `OPERATIONAL` status natively.
    - **Citadel Shielding**: All tenant modification paths are fortified behind `SessionKey` verifications matching the Master Enclave payload.

## [1.0.3] - 2026-03-04
### Fixed
- **[ANK-113] Server Borrow Order & Import Cleanup**:
    - **Tonic Request Fix (E0382)**: Resolved a "borrow of moved value" error in `AnkRpcServer` by extracting `CitadelAuth` extensions before consuming the request body with `into_inner()`.
    - **Import Optimization**: Removed unused imports (`WasiCtxBuilder`, `IpAddr`, `Duration`) in `ank-core` to ensure a clean, warning-free build in production.

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
