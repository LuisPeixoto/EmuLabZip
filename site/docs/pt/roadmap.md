---
source: e1df661a2578
description: O plano do EmuLabZip por fases, do que já foi lançado (v0.1 a v0.14) ao que vem a seguir até a versão 1.0, com os limites conhecidos de cada fase.
---
# Roadmap

O EmuLabZip cresce em fases. Cada fase termina com uma versão, e cada item dela chega funcionando em emuladores Android
e, onde a plataforma permite, em simuladores iOS. Dentro de cada fase, os itens seguem mais ou menos a ordem de prioridade. Ideias e pull
requests são bem-vindos; abra uma issue para discutir qualquer coisa grande antes.

## ✅ Fase 1: Conectar e observar (v0.1)

Descoberta automática de emuladores e dispositivos Android (cliente ADB em Rust puro) e de simuladores iOS iniciados (`simctl`) ·
seletor de dispositivo com status ao vivo · seletor de app com ids dos processos em execução · **Overview** com fatos de build, hardware, tela,
bateria, armazenamento, idioma e do host · pré-visualização da tela ao vivo e screenshots · **visualizador de log em streaming** para logcat e
o log unificado: filtros por nível, tag, texto e expressão regular, filtro por app, pausa, lista virtualizada que aguenta
milhares de linhas por segundo, exportação em texto simples e JSON-lines · linha de comando `emulabzip`: `devices`, `info`, `apps`,
`logs`, `screenshot`.

## ✅ Fase 2: Desempenho, memória e armazenamento (v0.2)

Gráficos ao vivo de CPU, memória, taxa de frames, frames travados e bateria do app selecionado · detalhamento de memória (Java, nativa,
gráficos, código, stack no Android; regiões da VM no iOS) com snapshots e um diff · heap dumps (`.hprof`) no Android e
grafos de memória (`.memgraph`) no iOS · navegador de arquivos da sandbox do app (`run-as` para apps Android debuggable, armazenamento
compartilhado, e os containers de dados, bundle e app group no iOS) · inspetor de SQLite com linhas paginadas e consultas
somente leitura · visualizadores de SharedPreferences, property list, JSON, imagem, texto e hex · `emulabzip perf`, `mem`, `heapdump`,
`files`, `pull` e `db`. Veja [Desempenho](performance.md), [Memória](memory.md) e [Storage](storage.md).

## ✅ Fase 3: Rede (v0.3)

Um proxy HTTP(S) independente escrito em Rust (`emulabzip-net`) que decifra HTTP/1.1 e HTTP/2 · roteamento com um clique de
um dispositivo Android (`adb reverse` e a configuração global de proxy) ou do Simulador iOS (o proxy do Mac, salvo e
restaurado exatamente) · instalação de certificado: o armazenamento do sistema em emuladores com root, um certificado de usuário guiado
nos demais, `simctl keychain` nos simuladores · lista de requisições com filtros, headers, corpos decodificados e tempos · hosts que
rejeitam o certificado passam direto · WebSocket e outras conexões tuneladas e listadas · exportação de HAR ·
`emulabzip net` e `emulabzip ca`. Veja [Rede](network.md).

## ✅ Fase 4: Traces (v0.4)

Traces de sistema do Perfetto com presets (CPU e escalonamento, frames e jank, memória, personalizado) · um resumo e um console SQL
apoiados no `trace_processor` oficial, baixado uma vez, fixado e verificado · flame graphs de CPU do `simpleperf`
(um app ou o sistema inteiro) · gravações do Instruments com `xctrace`, com um flame graph para o Time Profiler · abrir
traces na UI do Perfetto ou no Instruments · uma biblioteca de gravações · `emulabzip trace`. Veja [Traces](traces.md).

## ✅ Fase 5: Crashes e automação (v0.5)

Coleta de crashes, crashes nativos e ANRs (DropBox no Android, relatórios `.ips` no Simulador iOS) com stacks, os fatos
que o dispositivo registrou, dicas para simbolização e uma notificação quando um novo acontece · controles de localização,
aparência, barra de status, bateria, permissões, notificações e links · um **servidor MCP** para que assistentes de IA leiam
dispositivos, logs, crashes e traces e ajam sobre eles · plugins: seus próprios coletores como programas que imprimem JSON-lines,
ativados pelo usuário. Veja [Crashes](crashes.md), [Controles](controls.md), [Servidor MCP](mcp.md) e
[Plugins](plugins.md).

## ✅ Fase 6: Ferramentas avançadas de rede, layout e sessões (v0.6)

**Mocks, atrasos, bloqueios e breakpoints** em requisições e respostas · **mensagens de WebSocket**, incluindo
`permessage-deflate` · **importação de captura de pacotes** (`emulator -tcpdump`, `.pcap` e `.pcapng`) com HTTP, nomes de servidor TLS
e DNS · **HTTP/3** do proxy para os servidores · **inspeção de layout** e uma **auditoria de acessibilidade** (rótulos, áreas de
toque, contraste) no Android e no Simulador iOS · **sessões** gravadas em um único arquivo, e **modo CI** que
grava em torno da execução de um teste · **comparação de dois dispositivos**. Veja [Mocks](mocks.md), [WebSockets](websockets.md),
[Importação de captura](pcap.md), [Layout](layout.md), [Sessões](sessions.md), [Modo CI](ci.md) e [Comparar](compare.md).

## Fase 7: Desempenho e qualidade (v0.7) ✅

**Tempo de abertura do app**, a frio, morno e quente: `am start -W` e as linhas `Displayed` no Android, `simctl launch` com os signposts de
abertura do log unificado no iOS, repetido N vezes com a mediana exibida · **energia** por app: `dumpsys
batterystats` (CPU, wakelocks, rádio, GPS) no Android, o estado térmico e o tempo de CPU no simulador · um **localizador de vazamentos**
para heap dumps `.hprof`: tamanhos retidos, dominadores, e Activities e Fragments destruídos que ainda estão
alcançáveis, com o caminho até a raiz do GC · **orçamentos de desempenho** em um `emulabzip.toml` (tempo de abertura, pico de memória,
frames travados, número de requisições) que o `emulabzip ci` verifica, falhando a execução quando um é violado · **baselines**: compare uma
execução com a última boa e diga o que ficou mais lento.

## Fase 8: Por dentro dos frameworks (v0.8) ✅

**Flutter**: o serviço da Dart VM dos builds de debug (árvore de widgets, tempos de frames, memória) · **React Native**: Hermes e a
árvore do React pelo protocolo do depurador · **Jetpack Compose e SwiftUI** no Layout por meio das árvores de semântica, com
contagens de recomposição onde o Android as expõe · decodificação de rede para **gRPC e protobuf** (com arquivos `.proto`, ou
bruto) e **GraphQL**, com as operações nomeadas na lista · **editar e reenviar** uma requisição · **condições de rede** no
proxy: presets para 3G, Wi-Fi lento, latência alta, offline e perda de pacotes.

Ficou para depois nesta fase: **contagens de recomposição** (o Android Studio as obtém de um agente carregado no app,
que usa interfaces não públicas), os nomes dos tipos de view do próprio SwiftUI (a acessibilidade não os expõe) e
gravar as condições de rede em uma sessão.

## Fase 9: Stacks legíveis e relatórios de bug com um clique (v0.9) ✅

**Simbolização** de crashes e stacks: retrace do R8 e do ProGuard a partir do `mapping.txt`, dSYMs com `atos` em um Mac, e
**source maps** do Hermes e do Metro, para que as stacks de JavaScript do React Native apontem para o arquivo e a linha certos · arquivos de mapping
guardados por app e por versão, e `--mapping` na linha de comando · um **relatório de bug** com um clique: a sessão, um
screenshot, os detalhes do dispositivo, os logs filtrados e as requisições que falharam, reunidos em um único arquivo HTML ou abertos como
uma issue no GitHub, GitLab ou Jira.

## Fase 10: Linha do tempo e replay (v0.10) ✅

Uma **linha do tempo** que alinha toques, problemas de log, requisições, frames lentos, coletas de lixo e crashes em um único eixo de
tempo, em um relógio medido em relação ao dispositivo · **vídeo da tela** gravado em uma sessão em pedaços curtos (`screenrecord`
no Android, `simctl io recordVideo` no iOS), em sincronia com a linha do tempo no app desktop, e o trecho em torno do crash
adicionado aos relatórios de bug · **replay de rede**: uma sessão ou um arquivo HAR responde às requisições do app, que então roda offline contra
respostas gravadas, para bugs cujo backend mudou desde então e para execuções de CI determinísticas (`--replay`,
`--fail-on-miss`).

Ficou para depois nesta fase: toques no iOS (o simulador não os entrega a nada fora dele), jank por frame no iOS,
replay de WebSockets e chamadas gRPC (um arquivo HAR não guarda as mensagens e os trailers deles) e a correspondência de uma requisição
que carrega um timestamp ou um id aleatório.

## ✅ Fase 11: Estado do app como dado (v0.11)

**[Snapshots do estado do app](app-state.md)**: salve a sandbox inteira em um arquivo `.ezstate` e restaure (`run-as` e tar para
apps Android debuggable, o container de dados no simulador), com um backup automático do que uma restauração substitui ·
**[snapshots de emulador e o ciclo de vida do dispositivo](devices.md)**: criar, iniciar, parar, apagar os dados e excluir AVDs e simuladores,
e `avd snapshot save|load|delete` · um **[modo de escrita](storage-editing.md)** no Storage para linhas de SQLite e
SharedPreferences (property lists pela linha de comando), atrás de uma confirmação, com Undo, e a somente leitura continua
sendo o padrão · visualizadores para **DataStore, MMKV, AsyncStorage, Core Data e Realm** (com um leitor do formato de arquivo do Realm) · um
**diff de schema** entre dois estados, para testar migrações · **[estado do React Native](react-native-state.md)**: Redux, React
Query, contexto e stores registrados, com um log de ações do Redux.

Limites: colunas de Realm do tipo set, dictionary, mixed, decimal e UUID não são lidas; os tipos do MMKV são deduzidos; os nomes de
atributos do Core Data perdem as maiúsculas e minúsculas; AsyncStorage e Core Data de um dispositivo iOS foram testados em arquivos de fixture, não em um
simulador; o estado do Flutter não é lido.

## ✅ Fase 12: Qualidade além do desempenho (v0.12)

Uma **[auditoria de segurança e privacidade](security-audit.md)** enquanto o app roda: tráfego em texto claro, tokens e dados pessoais em logs e
preferências, componentes exportados (`dumpsys package`), as flags `debuggable` e `allowBackup`, exceções do App Transport Security
no `Info.plist` e domínios ausentes do manifesto de privacidade · um **[analisador de APK, AAB e IPA](package-size.md)**: tamanho por
pasta, dex e assets, um diff entre dois builds e um orçamento de tamanho no `emulabzip.toml` · **[verificações de localização](localization.md)**:
pseudo-locales (`en-XA`, `ar-XB`), controles de escala de fonte e tamanho da tela, uma varredura por idiomas e tamanhos, e texto que
não cabe encontrado pelo Layout · **mais verificações de acessibilidade**: ordem de foco e simulação de daltonismo (a escala
de texto pela varredura) · **[regressão visual](visual-regression.md)**: screenshots comparados com uma baseline, com uma
imagem de diferença e uma verificação no CI · um **[inspetor de analytics](analytics.md)**: eventos de Segment, Amplitude, Mixpanel e Firebase
decodificados pelo proxy, e o log de debug do Firebase.

Limites: a auditoria não enxerga o atributo `exported` (o `dumpsys` não o imprime) e julga texto por padrão; o analisador de tamanho
lê tamanhos, pastas e cabeçalhos dex, não bytecode, e ainda não lê um app instalado; o texto cortado é uma heurística;
o Android troca o idioma por app, não por dispositivo; o decodificador do Firebase foi testado em payloads feitos segundo o layout do Google,
não em tráfego real; não há ferramenta de analytics por MCP.

## ✅ Fase 13: Automação e apps híbridos (v0.13)

**WebViews** no Android pelo Chrome DevTools Protocol, sobre os sockets `localabstract` do ADB sem nada redirecionado: o console, o
JavaScript e as requisições delas dentro do EmuLabZip (as requisições na view Network, marcadas como WEBVIEW), e um atalho guiado
para o Web Inspector do Safari no iOS · **input** no Android (toque, deslize, texto, teclas e abrir um app) no app, na linha
de comando e no MCP, e **gravação de um fluxo** a partir do `getevent`, reproduzido de novo e exportado como um fluxo do Maestro ou um script de shell; o iOS
não tem interface pública para toques, então isso fica só no Android · um **testador de deep links** com histórico e fixados, que verifica
o `assetlinks.json` ou o `apple-app-site-association` do domínio, a chave de assinatura do app, o team id e o entitlement, o que o dispositivo
decidiu (`pm get-app-links`) e quem abre o link · **regras de rede programáveis**: map local e map remote, encadeamento de regras,
respostas verificadas contra um arquivo OpenAPI 3, e scripts de regra em WebAssembly isolado que só rodam depois que seus bytes exatos
são aprovados. Veja [Páginas web em apps](webviews.md), [Input e fluxos](automation.md), [Deep links](deep-links.md),
[Regras de rede](network-rules.md) e [Scripts de regra](rule-scripts.md).

Limites: a inspeção de WebView exige um app debuggable, e o iOS é um atalho para o Safari, não um console dentro do app; os fluxos gravam apenas
a tela de toque (não toques injetados nem texto digitado) e só rotulam um toque quando a tela foi lida a tempo; a verificação de
deep link em um simulador não consegue validar domínios como um dispositivo faz; as verificações de OpenAPI cobrem respostas JSON, não requisições; Map
local e Map remote não se aplicam a chamadas gRPC; os scripts não guardam nada entre chamadas; os assistentes podem listar WebViews, ler o
console, verificar links e reproduzir fluxos, mas não podem executar JavaScript em uma página nem aprovar um script.

## ✅ Fase 14: IA com contexto (v0.14)

O servidor MCP passou de 67 ferramentas sem estado para **118 ferramentas, 8 prompts e 13 recursos** (130 ferramentas com as mudanças ativadas) que
mantêm estado enquanto rodam: o **proxy e as requisições capturadas** (iniciar, conectar um dispositivo, filtrar e ler requisições, regras,
condições, replay, breakpoints, exportação de HAR, verificações de OpenAPI), **observações ao vivo** de logs, desempenho, toques e streams de WebView e
React Native, **gravação de uma sessão** e comparação de duas, **dados do app somente leitura** (SQLite, Realm, Core Data, preferências,
DataStore, MMKV, AsyncStorage), leitura de crashes com mappings guardados, criação de dispositivos, fluxos, baselines visuais, varreduras de idioma
e orçamentos de build · toda ferramenta tem um título e informa se lê, altera ou destrói · **prompts** para as tarefas comuns (explicar um crash, por que está lento, resumir uma sessão, reproduzir um bug, depurar uma requisição, verificar uma versão, verificar um link, simular um
backend) e **recursos** que um assistente pode ler (dispositivos, sessões, fluxos, a captura ao vivo, um guia) · **as escritas são opt-in**:
`emulabzip mcp --allow-writes` oferece as ferramentas que apagam, excluem, restauram ou reescrevem dados, e cada chamada pede à pessoa que
confirme pelo cliente; aprovar scripts e plugins, e o `ci`, nunca são oferecidos · um card **AI assistants** em Settings
com a configuração para Claude Code, Claude Desktop, Cursor e VS Code · uma [referência do MCP](mcp-tools.md) gerada que um
teste mantém em sincronia com o servidor. Veja [Servidor MCP](mcp.md).

Limites: editar linhas e preferências, desfazer uma edição, gravar um fluxo por toque e importar uma captura de pacotes não são oferecidos
por MCP (continuam no app e na linha de comando); uma ferramenta longa ainda não informa progresso; os frames de vídeo de uma sessão não
são devolvidos como imagens; um cliente que não consegue pedir confirmação não pode usar as ferramentas de escrita, a menos que `--no-confirm` seja informado;
as observações guardam os últimos 5.000 itens e terminam após dez minutos de inatividade.

## Fase 15: Dispositivos e plataformas (v0.15)

**iPhones e iPads físicos** pelas ferramentas que o Xcode traz (`devicectl`, `usbmuxd`): logs, apps, screenshots, crashes
e arquivos onde o iOS permite, e uma nota simples onde não permite · **Android por Wi-Fi**, com o pareamento da depuração sem fio
a partir do app · emuladores de **Wear OS e Android TV**, e simuladores de **watchOS, tvOS e visionOS**, com as
views que se aplicam a cada um · **vários dispositivos ao mesmo tempo**: logs e requisições de dois dispositivos em uma única lista, marcados por dispositivo.

## Fase 16: Equipe e ecossistema, 1.0 (v1.0)

**Comparação de sessões** lado a lado, e um **relatório HTML compartilhável** de uma sessão (um arquivo, sem servidor) · **bibliotecas
de regras compartilhadas**: um arquivo de regras mantido no repositório do app, carregado pelo app e pela linha de comando · uma **extensão do VS Code**
com dispositivos, logs e o servidor MCP no editor · um **catálogo de plugins**: plugins da comunidade listados com checksums, ainda
ativados um a um pelo usuário · uma **interface em português** e em outros idiomas · **atualizações**: atualização automática assinada no
app, e pacotes Homebrew, winget e Scoop · a **promessa da 1.0**: as opções e o JSON da linha de comando, as ferramentas e os prompts do MCP
e o formato `.ezsession` ficam estáveis e só mudam com uma versão major.

## Ideias

O que ainda não está em uma fase: layouts para apps que desenham a própria UI (jogos, canvas) · streaming de HTTP/3 para respostas muito grandes ·
plugins que leem sessões · um **SDK complementar** opcional (npm, Gradle, SwiftPM) para o que só o código dentro
do app consegue ver, como o estado de Redux, Zustand ou React Query, contagens de recomposição e feature flags, mantido separado do
padrão, que não instala nada no dispositivo.
