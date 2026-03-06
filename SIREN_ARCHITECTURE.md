# 🚨 Protocolo Siren (Cognitive Sensing) - Arquitectura SRE

Este documento define la estrategia SRE (Site Reliability Engineering) para el manejo de streams de audio (Voz) entre la Aegis Shell (React/Vite), el BFF (FastAPI) y el Aegis Neural Kernel (ANK-Rust).

## 1. El Problema del Bloqueo (OOM & Backpressure)
Al manejar *streaming* bidireccional de bytes de audio, los buffers en memoria pueden crecer infinitamente si el ritmo de ingesta (productor) supera el ritmo de procesamiento (consumidor). Si delegamos el buffering al BFF, enfrentamos un riesgo altísimo de OOM (Out Of Memory) y alta latencia.

## 2. Flujo de Datos Arquitectónico
El audio viaja en el siguiente flujo:
`Micrófono del Browser (MediaRecorder API)` -> `WebSocket (Chunks binarios)` -> `FastAPI (BFF)` -> `gRPC Bidireccional stream` -> `Rust Kernel (STT VAD)`

Respuestas del Kernel:
`Rust Kernel (TTS)` -> `gRPC Bidireccional stream` -> `FastAPI (BFF)` -> `WebSocket` -> `Web Audio API (React)`

## 3. Estrategia de Backpressure (Tubo Pasarela Asíncrono)
Bajo la filosofía **Zero-Panic** y **Thin Client**, el BFF *no debe* almacenar ni interpretar el audio. Actuará puramente como un **Tubo Pasarela Asíncrono (Passthrough)**.

### Implementación en FastAPI (BFF):
1. **Generador Asíncrono 1:1:** FastAPI transformará directamente cada evento del WebSocket en un evento gRPC iterativo usando `async for` y `yield`.
2. **Propagación Natural (HTTP/2 Flow Control):** El protocolo gRPC (montado sobre HTTP/2) implementa control de flujo (*Flow Control*). Si el puerto gRPC de Rust se congestiona (ej. el Worker STT se demora), la librería `grpc.aio` en Python detendrá temporalmente el `yield`. Esto hará que FastAPI deje de vaciar el socket del WebSocket, lo cual a su vez detendrá al navegador vía el control de congestión de TCP.
3. **Chunking Controlado:** Todo el payload viaja particionado como especifica el `proto/siren.proto` (`AudioChunk`) con secuencias (para evitar desórdenes).
4. **Timeout de Sesión:** Si el *stream* queda inactivo (VAD detecta silencio prologando), el Kernel forzará un cierre elegante (graceful shutdown) de esa sesión `session_id`.

## 4. Estructura del Contrato (gRPC)
En `proto/siren.proto` se ha definido un solo método:
`rpc StreamSession(stream ClientAudioStream) returns (stream ServerAudioStream);`

Esta interfaz desacopla por completo la señal binaria (PCM/Opus) del resto de los datos de sistema del `kernel.proto`, protegiendo el núcleo de comunicación de Aegis de posibles ataques de desbordamiento en operaciones de red.
