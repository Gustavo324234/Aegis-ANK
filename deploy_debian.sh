#!/bin/bash
# Aegis Neural Kernel (ANK) - Debian SRE Deployment Script
# Autor: Antigravity (SRE Lead)
# Versión: 1.0.0
# Descripción: Automatiza la preparación del host, compilación de plugins Wasm y el binario del Kernel.

set -euo pipefail

# --- CONFIGURACIÓN DE COLORES ---
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}[INFO] Iniciando Aegis Neural Kernel (ANK) Deployment Pipeline...${NC}"

# --- FASE 1: DEPENDENCIAS DE SISTEMA ---
echo -e "${YELLOW}--- Fase 1: Instalando dependencias de sistema (Apt) ---${NC}"
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    cmake \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    libsqlite3-dev \
    curl \
    git

# --- FASE 2: RUST TOOLCHAIN ---
echo -e "${YELLOW}--- Fase 2: Configurando Rust Toolchain ---${NC}"
if ! command -v cargo &> /dev/null; then
    echo -e "${BLUE}[INFO] Rust no detectado. Instalando via rustup...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo -e "${BLUE}[INFO] Rust detectado: $(cargo --version)${NC}"
fi

echo -e "${BLUE}[INFO] Añadiendo target WebAssembly (WASI)...${NC}"
rustup target add wasm32-wasi

# --- FASE 3: THE FORGE (BUILD WASM PLUGINS) ---
echo -e "${YELLOW}--- Fase 3: Compilando Aegis Standard Library (Wasm) ---${NC}"
mkdir -p ./plugins

# Entrar al workspace de plugins y compilar
cd plugins_src
cargo build --release --target wasm32-wasi
cd ..

# Desplegar binarios .wasm a la carpeta de ejecución del Kernel
echo -e "${BLUE}[INFO] Desplegando binarios .wasm a ./plugins/ ---${NC}"
cp plugins_src/target/wasm32-wasi/release/*.wasm ./plugins/
echo -e "${GREEN}[OK] Plugins Wasm listos en ./plugins/${NC}"

# --- FASE 4: KERNEL BUILD ---
echo -e "${YELLOW}--- Fase 4: Compilando Aegis Neural Kernel (Server) ---${NC}"
# Forzamos compilación en release para máximo rendimiento de inferencia y gRPC
cargo build --release -p ank-server

# --- FASE 5: SYSTEMD IGNITION (DAEMONIZATION) ---
echo -e "${YELLOW}--- Fase 5: Configurando Systemd Service (ank.service) ---${NC}"
INSTALL_DIR=$(pwd)
SERVER_BIN="$INSTALL_DIR/target/release/ank-server"

# Generar el archivo de servicio dinámicamente con rutas absolutas
sudo bash -c "cat <<EOF > /etc/systemd/system/ank.service
[Unit]
Description=Aegis Neural Kernel (ANK) Server
After=network.target

[Service]
Type=simple
User=$(whoami)
WorkingDirectory=$INSTALL_DIR
ExecStart=$SERVER_BIN
Restart=always
RestartSec=5
# Logging rotativo vía Journald
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF"

echo -e "${BLUE}[INFO] Recargando daemons y levantando servicio ank...${NC}"
sudo systemctl daemon-reload
sudo systemctl enable ank
sudo systemctl start ank
echo -e "${GREEN}[OK] Aegis Neural Kernel ahora es un servicio persistente.${NC}"

# --- VERIFICACIÓN FINAL ---
if [ -f "$SERVER_BIN" ] || [ -f "$SERVER_BIN.exe" ]; then
    echo -e "\n${GREEN}====================================================${NC}"
    echo -e "${GREEN}   ANK DEPLOYMENT SUCCESSFUL (SRE GRADE)${NC}"
    echo -e "${GREEN}====================================================${NC}"
    echo -e "${BLUE}Configuración de Producción:${NC}"
    echo -e "- Binario:  $SERVER_BIN"
    echo -e "- Service:  /etc/systemd/system/ank.service"
    echo -e "- Estado:   Iniciado y habilitado (boot-persistent)"
    echo -e "\n${YELLOW}Logs en tiempo real:${NC}"
    echo -e "${BLUE}journalctl -u ank -f${NC}"
    echo -e "${GREEN}====================================================${NC}"
else
    echo -e "${RED}[ERROR] El binario del kernel no se encontró tras la compilación.${NC}"
    exit 1
fi
