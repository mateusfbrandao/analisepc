<div align="center">
  <img src="assets/icons/hicolor/scalable/apps/io.github.analisepc.AnalisePC.svg" width="112" height="112" alt="Ícone Analise PC" />
  <h1>Analise PC</h1>
  <p><b>Diagnóstico Profundo de Hardware, Telemetria em Tempo Real, Detector de Gargalo, Otimização e Benchmark Nativo para Linux / GNOME</b></p>
  <p>
    Construído em <b>Rust</b> com <b>GTK 4</b>, <b>Libadwaita</b> e <b>Cairo</b> seguindo rigorosamente a <a href="https://developer.gnome.org/hig/">GNOME Human Interface Guidelines (HIG)</a>.<br/>
  </p>
</div>

---

## ✨ Navegação Semântica e Organização (9 Abas Especializadas)

A barra lateral do aplicativo adota o padrão de **Sidebars com Cabeçalhos de Seção da GNOME HIG**, separando a telemetria passiva das ferramentas ativas de diagnóstico e teste:

### 📡 SEÇÃO MONITORAMENTO

#### 1. 📊 Visão Geral e Telemetria em Tempo Real
- **Painel de KPIs Adaptativo (`FlowBox`):** Quatro cartões coloridos no topo com uso de Processador (%), Memória RAM (%), Temperatura de Pico (°C) e Energia/Uptime.
- **Sistema Operacional e Plataforma (No Topo):** Distribuição Linux, versão do Kernel, ambiente GNOME Shell, sessão gráfica (`Wayland`/`X11`), modelo do computador, fabricante/versão da Placa-Mãe e versão/data da BIOS UEFI via `/sys/class/dmi/id/`.
- **Processador e Carga Individual por Thread:** Modelo do processador, frequências atuais de clock (MHz), gráfico histórico de uso dos últimos 60 segundos e gráfico de barras verticais ao vivo para cada núcleo/thread lógica.
- **Pentes e Slots Físicos de Memória RAM (Sem `sudo`):** Identifica cada módulo instalado (`DIMM A`, `DIMM B`), tecnologia (`DDR4`/`DDR5`), velocidade (`MT/s`), formato (`SODIMM`/`DIMM`), fabricante JEDEC (Crucial/Micron, Kingston, Samsung, etc.), Rank, número de série e validação de canal **Dual Channel**.
- **Bateria e Energia:** Nível de carga, estado de alimentação, tensão (`V`), consumo elétrico (`W`), capacidade atual vs. projeto (`mAh`) e cálculo percentual de desgaste da célula.
- **Refrigeração do Chassi e Ventoinhas (Integrado):** Linha de pico de RPM com badge numérico, curva histórica em tempo real (60s) e tacômetro individual de cada ventoinha monitorada pelo kernel Linux.

#### 2. 💾 Armazenamento e Saúde S.M.A.R.T. Completa
- **Unidades Físicas e Partições:** Lista discos NVMe/SATA/SSD, número de série, revisão de firmware, estado de integridade e partições montadas com barras de ocupação.
- **Temperatura Operacional Dedicada:** Linha individual com termômetro, classificação térmica (*Excelente*, *Moderada* ou *Elevada*) e badge visual colorido (°C).
- **Telemetria S.M.A.R.T. Nativa via UDisks2 / D-Bus (Sem `sudo`):**
  - Percentual de **vida útil restante** e nível de desgaste do SSD.
  - **Reserva disponível (`Available Spare`)** e versão do protocolo NVMe.
  - **Volume Total Gravado (`TBW`)** e **Total Lido** em Terabytes (`TB`).
  - **Horas Ligado (`Power-On Hours`)**, ciclos de energia, desligamentos abruptos e contador de erros de mídia.

#### 3. 🎮 Placas de Vídeo (Estilo GPU-Z) e Controle NVIDIA Optimus / PRIME
- **Controle de Gráficos Híbridos (`prime-select`):** Linha dedicada com badge de status do perfil ativo (`ON-DEMAND`, `NVIDIA`, `INTEL`) e botões rápidos com *tooltips* explicativos de cada modo (Economia vs. Desempenho Máximo).
- **Comando PRIME Render Offload:** Botão de 1 clique para copiar o parâmetro de inicialização para Steam ou Lutris (`__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia %command%`).
- **Métricas Avançadas GPU-Z (iGPU + dGPU):**
  - Versão do driver em uso e versão da **VBIOS**.
  - **Barramento PCIe:** Geração e largura máximas vs. operação em economia de energia (ex: `PCIe Gen 4 x16 @ Gen 1 x8`).
  - **Clocks em Tempo Real:** Frequências atuais e Boost máximo de núcleo e memória de vídeo (MHz).
  - **VRAM, Consumo e Processos:** Memória dedicada alocada (`MiB`/`GiB`), consumo elétrico em Watts (`W`), estado de energia (`P-State`), APIs suportadas (Vulkan, OpenGL, CUDA, NVENC) e processos que mantêm a dGPU acordada.

#### 4. 🌐 Rede e Conectividade
- **Gráficos de Tráfego RX/TX em Tempo Real (60s):** Curvas auto-escaláveis de Download (RX) e Upload (TX) atualizadas a cada 1 segundo.
- **Interfaces Físicas Separadas por Tipo (Cabeada vs. Wi-Fi):** Cada adaptador possui um grupo dedicado contendo:
  - **IP:** Endereço IPv4 com máscara CIDR e badge de conexão ativa (`Conectada` / `Desconectada`).
  - **MAC:** Endereço físico do adaptador.
  - **Dispositivo:** Nome da interface (`enp3s0`, `wlp0s20f3`), link negociado e driver do kernel.
  - **Fabricante:** Nome do fabricante decodificado (ex: *Realtek Semiconductor*, *Intel Corporation*).
  - **Modelo:** Controlador de rede detalhado (ex: *RTL8111/8168 PCI Express Gigabit*, *Alder Lake-P CNVi WiFi 6*).
  - **Taxa de Transferência Dedicada:** Velocidade instantânea de download e upload (`KB/s`) e volume acumulado (`GiB ↓ / GiB ↑`).
- **Diagnóstico de Latência ICMP, Jitter e Perda de Pacotes:** Campo de entrada (`AdwEntryRow`) para aferir `Ping Médio/Mín/Máx`, `Jitter` e `% de Packet Loss` contra qualquer domínio ou IP com veredito de estabilidade para jogos online.

#### 5. 🖱️ Periféricos, Monitores, Áudio e Portas Seriais
- **Monitores e Telas DRM:** Detecta painéis internos (`eDP`) e monitores externos (`HDMI`, `DisplayPort`), resolução ativa e qual placa de vídeo gerencia cada saída.
- **Subsistema de Áudio (`PipeWire` / `WirePlumber`):** Servidor de áudio ativo, saídas de som (*Sinks*) e microfones (*Sources*), indicando o dispositivo padrão e níveis de volume.
- **Dispositivos USB com Filtro de Busca:** Mouses, teclados, microfones USB, webcams, telas auxiliares USB (ex: Turing Smart Screen / TURZX), leitores biométricos e adaptadores Bluetooth.
- **Portas Seriais (`ttyUSB` / `ttyACM`):** Placas Arduino, ESP32 e conversores USB-Serial (CH340, CP210x, FTDI).

---

### 🛠️ SEÇÃO FERRAMENTAS

#### 6. 🩺 Diagnóstico Automático do Hardware e Exportação de Laudo
- **Índice Global de Integridade (`0 a 100`):** Avalia automaticamente temperatura da CPU, contador de *Thermal Throttling* do pacote no kernel, operação em Dual Channel, saúde S.M.A.R.T., limites de TGP da GPU e desgaste da bateria.
- **Exportação de Laudo Técnico Completo (`.html` e `.json`):** Gera relatórios estilizados em Dark Mode (prontos para impressão/PDF) ou arquivos JSON diretamente na pasta pessoal (`~/AnalisePC_Laudo_<hostname>.html`).
- **Monitor de Eventos de Hardware do Kernel (`journalctl -k`):** Inspeciona o log do kernel Linux com botões de filtro (`Todos`, `Avisos`, `Críticos`) para eventos de CPU (`MCE`), PCIe (`AER`), GPU (`NVRM`), NVMe e memória (`EDAC`).

#### 7. 🕹️ Jogos, Detector de Gargalo e MangoHud
- **Detector de Gargalo (CPU vs. GPU vs. VRAM):** Analisa o equilíbrio de hardware para `1080p Competitivo`, `1080p AAA no Alto/Ultra` e `1440p Quad HD`, alertando sobre limitações de VRAM em placas de 4 GB.
- **Estimativa de FPS em Jogos:** Projeção de taxa de quadros para títulos populares (*CS2*, *Overwatch 2*, *Valorant*, *Forza Horizon 5*, *Cyberpunk 2077*, *Red Dead Redemption 2*, *Ghost Recon Breakpoint*).
- **Scanner de Biblioteca Steam:** Detecta jogos e camadas de compatibilidade Proton instalados localmente.
- **Configurador de Overlay (`MangoHud`):** Aplica com 1 clique o perfil de monitoramento em `~/.config/MangoHud/MangoHud.conf`.

#### 8. ⚙️ Otimizações, Limpeza de Disco e Vilões de Recursos
- **Modos de Energia Nativos (HIG Radio Buttons):** Alternância instantânea entre **Alto Desempenho**, **Equilibrado** e **Economia de Energia**.
- **Manutenção e Limpeza Inteligente de Disco:**
  - Limpeza de buffers e cache de páginas da RAM (`sync` + `drop_caches`);
  - Descarte de blocos livres em SSDs (`fstrim`);
  - Limpeza de **Logs do Systemd (`journald`)**, **Cache de pacotes APT (`.deb`)**, **Cache de Shaders gráficos (NVIDIA/Mesa/Vulkan)**, **Miniaturas do GNOME** e **Lixeira**.
- **Detector de "Vilões de Recursos":** Top 5 processos consumidores de CPU (`%`), Top 5 de Memória RAM (`MiB`) e processos ativos na GPU dedicada NVIDIA, com botão **"Encerrar"** para processos do usuário.
- **Gerenciador de Aplicativos na Inicialização (`Autostart`):** Chaves ativadoras para arquivos `.desktop` em `~/.config/autostart/`.

#### 9. ⚡ Suíte de Benchmarks e Teste de Estresse
- **Benchmark de Processador (`Rayon`):** Testes sintéticos **Single-Core** e **Multi-Core** com duração configurável por *SpinRow*.
- **Benchmark de Memória RAM (`GB/s` e `ns`):** Mede taxa real de transferência de leitura, escrita e cópia em paralelo fora do cache L3 (`256 MiB`) e tempo de acesso DRAM em nanossegundos.
- **Benchmark Físico de Disco/SSD (`MB/s`):** Mede taxa sequencial física com sincronização no disco (`fsync`) e descarte de cache de página.
- **Stress Test Contínuo com Trava Térmica Automática:** Aplica carga máxima em todos os núcleos e controladora de memória com monitoramento em tempo real a cada 500 ms, interrompendo imediatamente o teste caso a temperatura atinja o limite de segurança configurado (`70 °C` a `100 °C`).

---

## ⌨️ Atalhos de Teclado

| Atalho | Ação |
| :--- | :--- |
| `Ctrl + Shift + C` | Copiar resumo completo do hardware para a área de transferência |
| `Ctrl + E` | Exportar Laudo Técnico Completo em HTML (`~/AnalisePC_Laudo_<hostname>.html`) |
| `F1` | Abrir janela **Sobre o Analise PC** |
| `F10` | Abrir o Menu Principal na barra superior |

---

## 📦 Instalação e Execução

### 1. Instalação via Pacote `.deb` (Recomendado para Debian / Ubuntu / Pop!_OS / Linux Mint)

Baixe a versão mais recente do pacote `.deb` nas [Releases do GitHub](https://github.com/analisepc/AnalisePC/releases) e instale com o gerenciador de pacotes da sua distribuição:

```bash
# Instalação rápida via terminal:
sudo apt install ./analisepc_0.1.0_amd64.deb
```

*O aplicativo será adicionado automaticamente ao menu de programas do sistema sob o nome **Analise PC**, com suporte nativo a atualizações de ícones e temas do GNOME.*

### 2. Instalação via Flatpak

Baixe o pacote bundle `.flatpak` oficial ou instale localmente com o comando:

```bash
# Instalar o arquivo bundle distribuível:
flatpak install --user ./analisepc.flatpak

# Executar via Flatpak:
flatpak run io.github.analisepc.AnalisePC
```

---

## 🛠️ Para Desenvolvedores (Compilação e Modo Terminal)

### Pré-requisitos para compilar o código-fonte
```bash
sudo apt update
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev
```

### Executar em Modo de Desenvolvimento e Modo Terminal (`--cli`)

```bash
# Iniciar a interface gráfica nativa GNOME em modo Release:
cargo run --release

# Executar auditoria instantânea completa diretamente no terminal (sem abrir janela):
cargo run --release -- --cli
```

---

## 🗂️ Estrutura do Código-Fonte

```text
AnalisePC/
├── Cargo.toml                                 # Metadados do pacote e configurações do cargo-deb
├── Makefile                                   # Automação de compilação, instalação e pacote .deb
├── packaging/
│   ├── build-deb.sh                           # Script nativo de empacotamento .deb com dpkg-deb
│   └── flatpak/
│       └── io.github.analisepc.AnalisePC.yml  # Manifesto oficial Flatpak (GNOME 47 Runtime)
├── data/
│   ├── io.github.analisepc.AnalisePC.desktop  # Lançador do sistema (Desktop Entry)
│   └── io.github.analisepc.AnalisePC.metainfo.xml # Metadados AppStream para GNOME Software/Flathub
├── assets/
│   └── icons/hicolor/                         # Ícones do App (128x128 SVG) e ícones simbólicos (16x16)
└── src/
    ├── main.rs                                # Ponto de entrada GUI (Libadwaita) e CLI (--cli)
    ├── core/                                  # Coletores de hardware (sysfs, udev, D-Bus, procfs)
    │   ├── os_info.rs                         # Distribuição, Kernel, GNOME, Placa-Mãe e BIOS DMI
    │   ├── cpu.rs                             # Topologia de CPU, clocks e uso por thread
    │   ├── memory.rs                          # RAM, Swap e decodificação DMI de pentes físicos
    │   ├── disk.rs                            # Discos físicos, partições e S.M.A.R.T. via UDisks2
    │   ├── gpu.rs                             # Gráficos integrados e dedicados, PCIe, Clocks e VBIOS
    │   ├── peripherals.rs                     # Interfaces de rede (Cabeada/Wi-Fi), Monitores, Áudio e USB
    │   ├── sensors.rs                         # Sensores térmicos e tacômetros de ventoinhas (hwmon)
    │   ├── battery.rs                         # Telemetria da bateria e percentual de desgaste
    │   ├── diagnostics.rs                     # Avaliação de saúde (0-100), Throttling e logs do kernel
    │   ├── gaming.rs                          # Detector de gargalo, estimador de FPS e biblioteca Steam
    │   ├── exporter.rs                        # Gerador de laudos técnicos em HTML e JSON
    │   ├── optimizer.rs                       # Modos de energia, PRIME, Turbo Boost, limpeza e vilões
    │   └── stress.rs                          # Benchmarks de CPU, RAM, NVMe e Stress Test Térmico
    └── ui/
        ├── charts.rs                          # Gráficos vetoriais Cairo (60s) seguindo as cores do GNOME
        └── window.rs                          # Telas Libadwaita responsivas (SplitView, Breakpoints e Toasts)
```
