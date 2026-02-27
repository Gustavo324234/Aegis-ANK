🧠 Aegis Neural Kernel (ANK) - Master Specification
1. Visión General y Objetivos
ANK es un Sistema Operativo Cognitivo diseñado para orquestar Modelos de Lenguaje Grandes (LLMs) locales de bajo tamaño (1B - 8B parámetros) tratándolos como Unidades Aritmético-Lógicas (ALUs) probabilísticas, controladas por un motor de ejecución estrictamente determinista.
Objetivos Principales:
Reducción de Dimensionalidad: Permitir que IAs pequeñas resuelvan problemas masivos fragmentándolos en Grafos de Tareas Acíclicos (DAGs).
Erradicación del Context Overflow: Implementar el Virtual Context Manager (VCM) con paginación de memoria. El modelo nunca recibe el proyecto entero; recibe "Punteros de Memoria" (0x0A4F) y solicita la información vía Neural Syscalls.
Ejecución Determinista y Auto-Sanación (Ring 0): Ningún código llega al usuario sin haber compilado exitosamente. El Kernel ejecuta, captura errores (stderr) y obliga al modelo a parchar su propio código en un bucle cerrado.
Multitarea Cognitiva (Context Switching): Capacidad de pausar un proceso largo de razonamiento, guardar su estado (PCB - Process Control Block), atender una petición urgente del usuario, y reanudar el proceso original sin perder contexto.
2. Plazos y Hoja de Ruta Global (Roadmap 2026-2027)
Dado que este es un proyecto de varios años, lo dividiremos en Hitos (Milestones) manejables.
Q1-Q2 2026: Fase 1 - El Motor DAG y el PCB (Fundación)
Creación de la estructura del Kernel.
Aislar la capa de LLM.
Lograr que un prompt complejo se traduzca matemáticamente en un árbol de tareas independientes.
Q3-Q4 2026: Fase 2 - Paginación y Memoria Virtual (VCM)
Implementación de las Neural Syscalls.
El modelo aprende a pedir datos del disco en lugar de tenerlos en el prompt.
Pruebas de estrés: Mantener el contexto de un proyecto de 50,000 líneas de código con un modelo que solo soporta 2,000 tokens.
H1 2027: Fase 3 - The Forge (Ring 0 Execution & Self-Healing)
Integración de contenedores efímeros (Docker/MicroVMs).
Bucle TDD (Test-Driven Development) automatizado. La IA escribe tests, escribe código, y el Kernel itera hasta que pasen.
H2 2027: Fase 4 - Asimilación en Aegis Core
Reemplazo del antiguo brain.py monolítico de Aegis.
Despliegue como el cerebro central del ecosistema completo.
3. Tablero de Trabajo: Epics y Tickets (Fase 1)
Para no abrumarnos, nos enfocaremos únicamente en la Fase 1. Aquí están los tickets que debemos ir cerrando.
EPIC 1: Arquitectura Base y Bloques de Control
Objetivo: Definir las estructuras de datos puras en Python, sin conectar ninguna IA todavía.
[ANK-101] Diseñar la estructura del PCB (Process Control Block):
Qué hacer: Crear las clases/esquemas que definen un "Proceso Cognitivo". Debe tener PID, Estado (RUNNING, WAITING, SLEEPING), Prioridad y Memoria temporal (Registros).
[ANK-102] Implementar el Scheduler Base (Planificador):
Qué hacer: Escribir el bucle principal (while True) del sistema operativo. Debe tomar procesos de una cola de pendientes, asignarles un tiempo de ejecución (quanta) y devolverlos a la cola si no han terminado.
EPIC 2: Motor de Grafo (El Fragmentador)
Objetivo: Romper intenciones en pasos matemáticamente ejecutables.
[ANK-201] Estructura de Datos DAG (Directed Acyclic Graph):
Qué hacer: Crear el sistema de nodos. Cada nodo es una micro-tarea con dependencias (ej. Nodo B necesita el output del Nodo A para empezar).
[ANK-202] Generador de DAG Dummy:
Qué hacer: Crear un script que, dado un prompt en texto ("Crea una calculadora en Python"), genere un DAG duro/mockeado sin usar IA, solo para probar que el Scheduler del EPIC 1 puede recorrer el grafo en el orden correcto.
EPIC 3: Interfaz de ALU (Capa de Modelo Físico)
Objetivo: Conectar el Kernel a la "CPU" (Ollama).
[ANK-301] Driver de Ejecución Atómica:
Qué hacer: Un wrapper alrededor de las llamadas a Ollama/LLM que imponga restricciones estrictas. La temperatura, semilla (seed) y tokens máximos deben ser dictados por el Kernel, no por el usuario.
[ANK-302] Parser de Interrupciones (Syscall Catcher):
Qué hacer: Una función en el flujo de salida del LLM que escanee el texto en tiempo real buscando tokens especiales como [SYS_CALL_...]. Si lo detecta, detiene la inferencia inmediatamente y devuelve el control al Kernel.
4. Metodología: Qué hacer en cada proceso
Para asegurar que esto sea de clase mundial, no podemos programar "a lo loco". Seguiremos este flujo de trabajo estricto para cada Ticket:
Paso A: Definición de la API (Arquitectura en Papel)
Antes de escribir un archivo .py, definiremos cómo lucen los datos. Por ejemplo, antes de programar el PCB (ANK-101), escribiremos un JSON de cómo se vería ese PCB. Lo revisamos juntos para encontrar fallas lógicas.
Paso B: TDD Estricto (Desarrollo Guiado por Pruebas)
En sistemas deterministas que manejan IAs probabilísticas, los tests son vitales.
Antes de escribir el código del Scheduler, escribiremos un test que simule 3 tareas. El test debe fallar. Luego escribimos el código del Scheduler hasta que pase el test. Esto garantiza que el Kernel es de roca sólida.
Paso C: Implementación "Mockeada" (Sin IA)
Los primeros meses, simularemos a la IA. Crearemos funciones que devuelvan strings predefinidos simulando ser el LLM. ¿Por qué? Porque si conectamos la IA desde el día 1, no sabremos si un error es un bug de nuestro código o una alucinación del LLM. Primero probamos que la tubería no tenga fugas usando agua (mocks), luego le metemos presión (LLMs reales).
Paso D: Inyección de IA y Calibración del Prompt Base
Una vez que la tubería funciona con datos falsos, conectamos el LLM. Aquí la ingeniería de prompts no será para "charlar", será programación de bajo nivel. Enseñaremos al modelo el manual de instrucciones (Instruction Set Architecture - ISA) del Kernel.

## Filosofía de Diseño: ANK vs OpenClaw / AutoGPT

Sistemas contemporáneos como **OpenClaw** resolvieron la omnipresencia del agente (WhatsApp, Telegram, Cron jobs). Sin embargo, utilizan una arquitectura de **"Ejecución Lineal Basada en Oráculo"**:
1. Usuario envía petición compleja.
2. El sistema ensambla un prompt masivo con el historial y las herramientas.
3. Se depende de un LLM masivo y costoso (ej. Claude/GPT-4) para entender, planificar y ejecutar de una vez.

**Aegis Neural Kernel (ANK)** introduce el **"Procesamiento Cognitivo Fragmentado"**:
- **No hay Oráculos, hay ALUs:** El LLM (incluso uno pequeño de 1.5B) es tratado como una unidad lógica. ANK nunca le pasa el problema completo.
- **Grafo de Tareas (DAG):** El Kernel descompone el prompt en un árbol de micro-tareas independientes.
- **VCM (Virtual Context Manager):** En lugar de enviar un prompt de 30k tokens, ANK inyecta punteros de memoria dinámicos, manteniendo el LLM rápido, barato y sin "olvidos" en el medio del contexto.
- **Ring 0 Isolation:** La IA no ejecuta comandos de cara al usuario. ANK compila/ejecuta en un entorno aislado (Sandbox), lee el `stderr` y fuerza a la IA a auto-repararse antes de entregar el resultado final al usuario.

## Capa de Abstracción Cognitiva (Cognitive HAL - cHAL)

Para garantizar la longevidad del Kernel y su adaptabilidad a la Ley de Moore de la IA, ANK implementa un **Cognitive Hardware Abstraction Layer (cHAL)**.

1. **Agnosticismo de Modelos:** El Kernel y el Scheduler operan con "Clases de Interfaces" (ej. `Logic_ALU`, `Code_ALU`, `Fast_Router_ALU`). Nunca se hace referencia a modelos específicos (como Llama o Mistral) en el código core.
2. **Hardware-Aware Routing:** El cHAL monitorea la RAM/VRAM en tiempo real. Si el sistema está bajo presión de memoria, el cHAL puede degradar la petición a un modelo más pequeño o encolar la tarea hasta que la VRAM se libere.
3. **Escalabilidad Dinámica:** Si el usuario migra su entorno de una laptop (Tier 1) a un servidor con múltiples GPUs (Tier 3), ANK detecta el hardware en el arranque y pasa de una ejecución secuencial agresivamente paginada, a una ejecución multi-hilo con modelos de 32B o 70B residentes en VRAM.