🗺️ ANK Project Tracking (Kanban Mode)
Este archivo es la fuente de verdad del progreso del Aegis Neural Kernel. El programador debe mover los tickets de sección y actualizar las versiones según se complete el trabajo.
🟩 DONE (Completado)
Actualmente el sistema está en fase de especificación. No hay tareas cerradas.
🟦 IN PROGRESS (En Desarrollo)
[ANK-100] Setup del Workspace de Rust
Estado: Definido. Esperando inicialización de Cargo.
Versión Objetivo: v0.1.0
🟥 TO DO (Pendiente - Fase 1: Engine Core)
EPIC 1: Infraestructura y Red
[ANK-101] Definición del Contrato Protobuf
Crear kernel.proto con estructuras PCB, TaskRequest y streams de eventos.
[ANK-103] Implementación de ANK-Bridge (gRPC Server)
Levantar servidor Tonic y mapear endpoints a canales internos.
EPIC 2: El Corazón del Kernel
[ANK-102] Cognitive Scheduler (Tokio Loop)
Implementar Priority Queues y lógica de Context Switching asíncrono.
[ANK-107] The Scribe: Trazabilidad y Versiones
Integración con Git/SemVer para registro automático de cambios por IA.
EPIC 3: Capa de Inferencia (cHAL)
[ANK-104] Cognitive HAL & Inference Traits
Definir la abstracción para que el Kernel sea agnóstico al modelo.
[ANK-105] Native Driver (llama-cpp-2)
Implementar inferencia local de alto rendimiento con GBNF Grammar.
EPIC 4: Gestión de Memoria
[ANK-106] Virtual Context Manager (VCM)
Sistema de paginación semántica y resolución de punteros URI.
[ANK-108] Integración LanceDB
Persistencia de memoria de largo plazo (Swap Cognitivo).
