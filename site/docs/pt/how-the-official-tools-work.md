---
source: 3928a9800ad4
description: O que descobrimos sobre como o Xcode Instruments e os profilers do Android Studio coletam dados, e quais interfaces públicas o EmuLabZip usa no lugar.
---
# Como as ferramentas oficiais funcionam

Antes de escrever o EmuLabZip, estudamos como o Xcode Instruments e os profilers do Android Studio coletam seus dados. O
estudo foi estático: olhamos os pacotes de aplicativo distribuídos, as bibliotecas que eles usam, as classes e os pacotes
dentro dos arquivos JAR e as strings dos binários que rodam no dispositivo. Nada foi alterado nem redistribuído. O objetivo era
entender **quais fontes de dados existem** e, depois, escolher as que uma ferramenta de código aberto consegue usar por meio de
interfaces públicas e documentadas.

## Xcode Instruments

| Área | O que encontramos |
|------|-------------------|
| Linguagens e UI | Swift e Objective-C, AppKit com SwiftUI, mais frameworks privados de gráficos e de árvore de chamadas |
| Plugins | Pacotes `.xrplugin` e pacotes de instrumentos `.instrdst` (Networking, HTTP Tracing, SwiftUI, Hangs, Hitches, System Trace…) |
| Transporte | **DTX**, um protocolo RPC privado, sobre sockets, portas Mach ou memória compartilhada, com compressão |
| Agente no dispositivo | O `DTServiceHub` expõe canais como `sysmontap` (estatísticas de processos e do sistema), `coreprofilesessiontap` (trace do kernel), `objectalloc`, `vmtracking` e `activitytracetap` (os_log e signposts) |
| Fontes de dados | Trace do kernel (`ktrace`, `kperf`), o sistema de log unificado e signposts, simbolização |
| Rede | Estatísticas de conexão (`netstats`) e o rastreamento de HTTP do CFNetwork emitido pelo log unificado, não um proxy |
| Linha de comando | `xctrace record` e `xctrace export` |

## Xcode e o Simulator

O `simctl` controla o **CoreSimulator** por meio de um serviço XPC. Ele consegue listar e iniciar dispositivos, executar comandos dentro
de um simulador (`simctl spawn … log stream`), capturar a tela (`simctl io screenshot`, `recordVideo`), listar apps e seus
containers, sobrescrever a barra de status, definir a localização, enviar notificações push e adicionar certificados raiz. Um app
de simulador é um processo comum do macOS, então o host consegue ler diretamente o uso de CPU e de memória dele.

## Android Studio

| Área | O que encontramos |
|------|-------------------|
| Plataforma | IntelliJ Platform no JetBrains Runtime, Kotlin e Java, Swing com Compose for Desktop para a UI mais nova do profiler |
| Acesso ao dispositivo | ddmlib e adblib (ADB), e a API gRPC do emulador, descoberta a partir dos arquivos `avd/running/pid_*.ini` |
| Profilers | Um *pipeline de transporte*: grpc-java e protobuf entre o Studio e um daemon no dispositivo |
| Daemon no dispositivo | `transport` (antes `perfd`), enviado para `/data/local/tmp/perfd/`. Ele executa `dumpsys meminfo --checkin`, `dumpsys SurfaceFlinger --latency`, `perfetto`, `simpleperf`, `am dumpheap` e carrega um agente JVMTI em apps debuggable |
| Traces | Perfetto (`trace_processor` como serviço gRPC), relatórios do simpleperf, traces de métodos do ART |
| Memória | Heap dumps `.hprof` lidos com o Shark, alocações nativas pelo heapprofd no Perfetto |
| Rede | Antes, reescrita de bytecode do OkHttp e do `HttpURLConnection` com ASM (`profilers-transform.jar`); hoje, o inspetor de rede do App Inspection, que se encaixa nas pilhas HTTP dentro de apps debuggable |

## O que o EmuLabZip usa

Usamos apenas **interfaces públicas e documentadas**. O protocolo DTX da Apple e os agentes no dispositivo do Google são privados,
mudam entre versões e não temos o direito de redistribuí-los.

| Dados | Android | Simulador iOS |
|-------|---------|---------------|
| Dispositivos | Protocolo host do ADB (`host:devices-l`), falado diretamente em Rust | `xcrun simctl list devices -j` |
| Fatos do dispositivo | `getprop`, `wm`, `dumpsys battery`, `/proc` | JSON do `simctl`, lista de runtimes, `defaults` dentro do simulador |
| Apps | `pm list packages`, `ps` | `simctl listapps`, `launchctl list` dentro do simulador |
| Logs | `logcat -v threadtime` | `log stream --style ndjson` dentro do simulador |
| Screenshots | `screencap -p` | `simctl io screenshot` |
| CPU e memória (Fase 2) | `/proc/<pid>/stat`, `dumpsys meminfo` | Estatísticas de processo do host |
| Frames (Fase 2) | `dumpsys gfxinfo framestats` | Signposts e `xctrace` (Fase 4) |
| Traces (Fase 4) | Perfetto, simpleperf | `xctrace record` e `export` |
| Rede (Fase 3) | Proxy HTTP com uma CA de usuário, `emulator -tcpdump` | Proxy do host, `simctl keychain add-root-cert` |

Isso mantém o EmuLabZip pequeno, seguro para rodar em qualquer imagem de emulador e estável entre as atualizações do Xcode e do
Android Studio. Também significa que nada é instalado no dispositivo: toda consulta é algo que você mesmo poderia digitar no
`adb shell` ou no `xcrun simctl`.
