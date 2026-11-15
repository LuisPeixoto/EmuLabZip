---
source: a1e5e440e93b
description: Como o EmuLabZip é organizado em um workspace Cargo, com crates pequenos atrás de uma trait comum, dois front ends e as regras de projeto.
---
# Arquitetura

O EmuLabZip é um workspace Cargo. O código de cada plataforma fica em crates pequenos atrás de uma única trait, e dois front ends
(o app desktop e a CLI) usam esses crates do mesmo jeito.

```text
                ┌──────────────────────┐      ┌──────────────────┐
                │  apps/desktop        │      │  emulabzip-cli   │
                │  Svelte UI ◄─► Tauri │      │  (clap)          │
                └──────────┬───────────┘      └────────┬─────────┘
                           │        Lab (routes by platform)
                           └───────────────┬───────────┘
                                 ┌─────────┴─────────┐
                      Backend    │                   │   Backend
                ┌────────────────┴───┐        ┌──────┴──────────────┐
                │ emulabzip-android  │        │ emulabzip-apple     │
                │ logcat, getprop,   │        │ simctl, log stream, │
                │ dumpsys, pm, ps    │        │ listapps, plutil    │
                └─────────┬──────────┘        └──────┬──────────────┘
                ┌─────────┴──────────┐               │
                │ emulabzip-adb      │          xcrun simctl
                │ ADB host protocol  │
                └─────────┬──────────┘
                    adb server :5037
```

## Crates

| Crate | Responsabilidade |
|-------|------------------|
| `emulabzip-core` | O modelo de dados (`Device`, `App`, `InfoSection`, `LogEntry`), filtros de log, um ring buffer limitado, o tipo `Stream` e a trait `Backend`. Nenhum código de plataforma. |
| `emulabzip-adb` | Um cliente assíncrono do protocolo host do ADB: listagem e acompanhamento de dispositivos, `shell:`, `exec:` e o protocolo de arquivos `sync:`. |
| `emulabzip-android` | Implementa `Backend` para Android sobre o `emulabzip-adb`. Parsers para `getprop`, logcat, `dumpsys battery`, `wm`, `ps` e `pm`. |
| `emulabzip-apple` | Implementa `Backend` para o iOS Simulator com `xcrun simctl`. Parsers para a lista de dispositivos, logs em ndjson e listas de apps. |
| `emulabzip-audit` | Auditorias de qualidade que olham o app de fora: as regras de segurança e privacidade e a tabela de padrões de segredos com redação, os tipos `Finding` e `Audit`, a coleta das entradas de um dispositivo por meio do `Lab`, o analisador de APK, AAB e IPA com a comparação de tamanho e o orçamento, a comparação de imagens e as baselines, e a varredura de idioma e tamanho de texto. |
| `emulabzip-storage` | Visualizadores que não dependem de plataforma: detecção de tipo de arquivo, um inspetor SQLite somente leitura, SharedPreferences e property lists, Preferences e Proto DataStore, MMKV, AsyncStorage, Core Data e um leitor do formato de arquivo do Realm; o **editor** (linhas de SQLite, preferências, property lists, com backup e desfazer); o formato de snapshot `.ezstate` e o diff de schema. |
| `emulabzip-net` | O proxy de depuração: uma autoridade certificadora, interceptação de TLS, encaminhamento de HTTP/1.1 e HTTP/2, gRPC em streaming, classificação e decodificação de GraphQL e gRPC, reenvio, condições de rede (e HTTP/3 para servidores por meio de `quinn` e `h3`), regras que simulam, atrasam, bloqueiam ou seguram requisições, **replay** de uma gravação HAR (um `ReplaySet` que o proxy consulta antes de qualquer servidor), um parser de frames de WebSocket, um leitor de capturas de pacotes, um armazém limitado de fluxos com um feed de mudanças, decodificação de corpos e exportação de HAR. Nenhum código de plataforma. |
| `emulabzip-trace` | Dar sentido às gravações: o modelo do flame graph, parsers para relatórios do simpleperf e exportações do `xctrace`, configurações do Perfetto, o trace processor (download, SQL, resumos), o servidor de uso único para a UI do Perfetto e a biblioteca de gravações. Nenhum código de plataforma. |
| `emulabzip-frameworks` | Olhar por dentro de apps Flutter e React Native: um pequeno cliente JSON sobre WebSocket (`Rpc`), o serviço da Dart VM (árvore de widgets, frames, memória) e o Metro com Hermes (árvore do React, heap, console e o **estado** de Redux, React Query, contextos e stores registrados, com um log de ações e um diff de estado). Alcança uma porta do dispositivo por `Lab::connect_port`, então nenhum código de plataforma fica aqui. |
| `emulabzip-symbols` | Tornar legíveis as stacks de crash: o retracer de `mapping.txt` do R8 e do ProGuard (com frames inline), source maps com um decodificador VLQ (mapas compostos do Metro e do Hermes), consultas de dSYM por meio do `atos`, o armazém por app e por versão desses arquivos e `symbolicate`, que aplica o que servir a um crash e diz o que falta. Trabalha com texto e números, então roda em arquivos sem dispositivo. |
| `emulabzip-report` | Relatórios de bug: reúne o dispositivo, um screenshot, o log, as requisições que falharam e os crashes legíveis (de um dispositivo ou de uma sessão), grava um único arquivo HTML autocontido e um resumo em Markdown, e monta links de issue pré-preenchidos para GitHub, GitLab e Jira. Não envia nada. |
| `emulabzip-heap` | Ler heap dumps `.hprof` e encontrar vazamentos: um parser de duas passadas, o grafo de objetos, dominadores e tamanhos retidos, e as regras de vazamento. Nenhum código de plataforma e nenhuma dependência, então também roda em um dump vindo de arquivo. |
| `emulabzip-session` | Sessões: o formato `.ezsession` (versão 2, com o relógio do dispositivo), um gravador que acompanha um dispositivo por meio do `Lab` (logs, amostras, crashes, screenshots, toques, frames lentos e pedaços de vídeo), um leitor que também abre a pasta de uma gravação interrompida e ainda abre arquivos da versão 1, o resumo, a **linha do tempo** que põe todos os streams em um único eixo, o carregador de uma gravação para replay, os orçamentos do `emulabzip.toml` e a comparação com uma baseline. |
| `emulabzip-mcp` | O servidor MCP: ferramentas sobre o `Lab`, servidas na entrada e na saída padrão com `rmcp`. |
| `emulabzip-plugin` | Plugins: manifestos, a etapa de aprovação, o protocolo JSON-lines e o executor. Nenhum código de plataforma. |
| `emulabzip-cli` | O comando `emulabzip`. |
| `apps/desktop` | O shell Tauri 2 (`src-tauri`) e a interface Svelte 5 (`src`). |

## A trait de backend

```rust
#[async_trait]
pub trait Backend: Send + Sync {
    fn platform(&self) -> Platform;
    async fn is_available(&self) -> bool;
    async fn devices(&self) -> Result<Vec<Device>>;
    async fn info(&self, id: &str) -> Result<Vec<InfoSection>>;
    async fn apps(&self, id: &str) -> Result<Vec<App>>;
    async fn logs(&self, id: &str, options: LogOptions) -> Result<Stream<LogEntry>>;
    async fn screenshot(&self, id: &str) -> Result<Vec<u8>>;

    // Optional: the default implementations return `Error::Unsupported`.
    async fn perf(&self, id: &str, options: PerfOptions) -> Result<Stream<PerfSample>>;
    async fn memory(&self, id: &str, app: &str) -> Result<MemoryReport>;
    async fn heap_dump(&self, id: &str, app: &str) -> Result<HeapDump>;
    async fn storage_roots(&self, id: &str, app: &str) -> Result<Vec<StorageRoot>>;
    async fn list_files(&self, id: &str, app: &str, root: &str, path: &str) -> Result<Vec<FileEntry>>;
    async fn read_file(&self, id: &str, app: &str, root: &str, path: &str) -> Result<Vec<u8>>;
    async fn trace_templates(&self, id: &str) -> Result<Vec<String>>;
    async fn record_trace(&self, id: &str, request: &TraceRequest, out_dir: &Path, stop: watch::Receiver<bool>) -> Result<TraceFile>;
    async fn clock(&self, id: &str) -> Result<DeviceClock>;
    async fn input(&self, id: &str) -> Result<Stream<InputEvent>>;
    async fn slow_frames(&self, id: &str, app: &str) -> Result<Stream<SlowFrame>>;
    async fn record_video(&self, id: &str, out_dir: &Path, segment_secs: u32, stop: watch::Receiver<bool>, segments: mpsc::UnboundedSender<VideoSegment>) -> Result<()>;
    async fn crashes(&self, id: &str, app: Option<&str>) -> Result<Vec<CrashSummary>>;
    async fn crash(&self, id: &str, crash_id: &str) -> Result<CrashReport>;
    async fn controls(&self, id: &str, app: Option<&str>) -> Result<ControlInfo>;
    async fn control(&self, id: &str, control: &Control) -> Result<()>;
    async fn network_status(&self, id: &str) -> Result<NetworkStatus>;
    async fn connect_network(&self, id: &str, port: u16) -> Result<NetworkStatus>;
    async fn disconnect_network(&self, id: &str) -> Result<NetworkStatus>;
    async fn install_ca(&self, id: &str, pem: &str, der: &[u8]) -> Result<CaInstall>;
}
```

O `Lab` guarda todos os backends e roteia as chamadas por `Target { platform, id }`. A falta de uma ferramenta (`adb`, `xcrun`) torna
um backend indisponível sem afetar os outros.

## Streams

Os logs são produzidos por uma tarefa em segundo plano e entregues por um canal limitado (`Stream<T>`). Descartar o stream
interrompe a tarefa, que fecha o socket do ADB ou encerra o processo filho do `log stream`, então nada sobrevive a quem lê.

No app desktop, cada stream é uma *sessão* pertencente ao estado do Tauri. As linhas são agrupadas a cada 100 ms e enviadas à
interface por um canal IPC do Tauri. Um lote tem no máximo 4.000 linhas; o excedente é contado e informado como descartado,
em vez de atrasar o dispositivo ou a UI. A interface mantém o próprio buffer limitado e renderiza apenas as linhas visíveis.

## Um único eixo de tempo

Cada stream marca o tempo do seu jeito (o log imprime a hora local do dispositivo sem o ano, as amostras contam a partir do
início delas, toques e frames contam a partir da inicialização do dispositivo), então uma gravação guarda um `DeviceClock`: o
relógio de parede do dispositivo comparado ao deste computador, o fuso horário dele e o momento em que o relógio monotônico
marcava zero. O `timeline::build` do `emulabzip-session` usa esse relógio para pôr toques, problemas de log, requisições, frames
lentos, coletas de lixo, crashes, screenshots e vídeo em um único eixo, em milissegundos desde o início. Um frame lento é
posicionado com o `Uptime` que o dump do `gfxinfo` imprime, que é o relógio em que os tempos dos frames estão. O vídeo da tela
é gravado em pedaços curtos (veja [Linha do tempo e vídeo da tela](timeline.md)), cada um informado quando fica completo, então uma
gravação interrompida guarda o que já tinha.

## O proxy

O `emulabzip-net` não sabe nada sobre dispositivos. Os backends só decidem como um dispositivo chega até ele e como passa a
confiar no certificado; a CLI e o app desktop iniciam o proxy e entregam a ele um `FlowStore`.

```text
 device ──► 127.0.0.1:<port> ──► proxy ──► the internet
                                   │
              CONNECT, first byte is 0x16 (TLS)?
                 yes ─► terminate TLS with a leaf for the SNI name, serve HTTP/1.1 or HTTP/2,
                        forward over a new TLS connection, record the flow
                 no  ─► copy bytes both ways, record a tunnel
              handshake refused ─► remember the host, pass it through next time
```

Um **replay** é mais uma etapa antes de o servidor ser consultado: depois das regras, o `forward` procura a requisição no
`ReplaySet` e responde a partir dele, ou responde 504 quando ela não está lá. Enquanto uma gravação é reproduzida, os caminhos
que não podem ser respondidos a partir de um HAR (um túnel que o proxy não consegue decodificar, um WebSocket, uma chamada gRPC,
um reenvio) são recusados, então nada chega à rede. Veja [Reproduzindo uma gravação](replay.md).

Os corpos de resposta são transmitidos ao cliente enquanto uma cópia é gravada (até 10 MB), então downloads e event streams
continuam funcionando. Os corpos de requisição são lidos por inteiro antes do encaminhamento. Os upgrades de WebSocket são
tunelados depois do handshake, e os bytes também alimentam um parser de frames que grava as mensagens.

Antes de uma requisição ser encaminhada, o proxy consulta as **regras** (a primeira que combinar vence): um mock responde na
hora, um atraso espera, um bloqueio falha e um breakpoint segura a requisição, ou a resposta, em um registro até que a UI
continue. Com HTTP/3 ligado, um host que anunciou `Alt-Svc: h3` é alcançado por QUIC, com fallback para TCP.

## Caminhos de armazenamento

Os caminhos de arquivo são sempre relativos a uma *raiz de armazenamento* escolhida pelo backend (`data`, `external`, `bundle`…). O
`Lab` rejeita caminhos absolutos e `..` antes de um backend vê-los, e o backend de iOS também resolve links simbólicos e
recusa qualquer coisa que acabe fora do container.

## Regras de projeto

- **Só interfaces públicas.** Tudo o que o EmuLabZip lê é algo que você conseguiria obter com `adb shell` ou `xcrun simctl`.
  Veja [Como as ferramentas oficiais funcionam](research/how-the-official-tools-work.md).
- **Sem injeção de shell.** Os valores que chegam ao shell do dispositivo são colocados entre aspas (`emulabzip_adb::quote`), os nomes
  de pacote são validados e o `simctl` é invocado sem shell.
- **Agrupe as idas e vindas ao dispositivo.** O Overview é uma única chamada de shell no Android; os parsers dividem a saída.
- **Os parsers são testados com saída real.** Fixtures capturadas de emuladores e simuladores ficam ao lado de cada crate.
- **Só escritas explícitas.** Os arquivos são gravados apenas depois de uma caixa de diálogo de salvar ou de uma flag da CLI.
