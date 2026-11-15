---
source: 951822beac6a
description: "O comando emulabzip dá os mesmos dados do app desktop em um terminal ou script: dispositivos, logs, desempenho, rede, sessões, controles e muito mais."
---
# Linha de comando

O comando `emulabzip` dá os mesmos dados do app desktop, em um terminal ou em um script. É um único binário estático, sem
dependências de runtime além de `adb` e Xcode.

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

Arquivos pré-compilados para macOS, Linux e Windows estão anexados a cada
[release](https://github.com/LuisPeixoto/EmuLabZip/releases).

## Escolhendo um dispositivo

Todo comando, exceto `devices`, trabalha com um dispositivo. Quando só um está em execução, ele é usado automaticamente. Caso
contrário, passe `--device` (ou defina `EMULABZIP_DEVICE`) com um serial do adb, um UDID de simulador, um nome ou parte de um
nome:

```bash
emulabzip --device pixel info
emulabzip -d "iPhone 17" logs
```

## Comandos

### `emulabzip devices`

```text
PLATFORM  NAME       OS                   STATE   ID
Android   Pixel 9    Android 16 (API 36)  online  emulator-5554
iOS       iPhone 17  iOS 26.2             online  F8CE84C1-04D1-4E33-B52B-02B59E8A09C5
```

`--json` imprime a lista como JSON. O alias `emulabzip ls` faz o mesmo.

### `emulabzip info`

Imprime os mesmos grupos do Overview (dispositivo, software, hardware, tela, bateria, locale). `--json` imprime um objeto de
objetos.

### `emulabzip apps`

Lista os apps do usuário com sua versão e id de processo. `--all` adiciona os apps do sistema, `--running` mantém só os que
estão em execução.

### `emulabzip logs`

Acompanha o log do dispositivo, colorido por nível, até você pressionar <kbd>Ctrl+C</kbd>.

| Opção | Significado |
|--------|---------|
| `-a, --app <id>` | Só as linhas deste app. O id do processo é atualizado a cada alguns segundos, então as reinicializações são acompanhadas. |
| `-l, --level <level>` | Nível mínimo: `verbose`, `debug`, `info`, `warn`, `error`, `fatal` ou uma letra. |
| `-t, --tag <text>` | A tag contém o texto. |
| `-g, --grep <text>` | A mensagem ou a tag contém o texto. |
| `-E, --regex` | Trata `--grep` como uma expressão regular. |
| `--tail <n>` | Linhas recentes a imprimir primeiro (Android, padrão 100). |
| `--json` | Um objeto JSON por linha, pronto para o `jq`. |

```bash
emulabzip logs --app com.example.app --level warn
emulabzip logs -g 'skipped \d+ frames' -E
emulabzip logs --json | jq -r 'select(.level == "error") | .message'
```

### `emulabzip screenshot`

Salva um PNG da tela. `-o, --output <file>` escolhe o caminho; por padrão o arquivo recebe o nome do dispositivo e da hora.

### `emulabzip perf`

Imprime uma linha por amostra: CPU do app e do dispositivo, memória do app e do dispositivo, taxa de quadros, quadros com
jank e bateria.

| Opção | Significado |
|--------|---------|
| `-a, --app <id>` | App a amostrar. Sem ele, só valores do dispositivo inteiro. |
| `-i, --interval <ms>` | Tempo entre amostras (padrão 1000, mínimo 250). |
| `--json` | Um objeto JSON por amostra. |

### `emulabzip mem <app>`

Um detalhamento da memória: PSS por categoria no Android, footprint por região de VM no iOS. Veja [Memória](memory.md).

### `emulabzip heapdump <app>`

Salva um heap dump `.hprof` (Android, apps debuggable) ou um grafo de memória `.memgraph` (iOS). `-o` escolhe o arquivo. Com
`--analyze` ele também procura vazamentos, veja [Encontrando vazamentos](leaks.md).

### `emulabzip heap analyze <file>`

Procura vazamentos em um `.hprof` (qualquer plataforma) ou em um `.memgraph` (em um Mac). `--json`; `--top <n>` define quantos
objetos e classes listar; `--all-heaps` olha além do heap do próprio app. Veja [Encontrando vazamentos](leaks.md).

### `emulabzip startup <app>`

Mede quanto tempo o app leva para abrir. `-k, --kind cold|warm|hot` (padrão `cold`), `-r, --runs <n>` (1 a 50, padrão 5),
`--json`. Veja [Tempo de abertura do app](startup.md).

### `emulabzip energy <app>`

Mede a energia que o app usa enquanto você o usa. `-s, --seconds <n>` (padrão 30; <kbd>Ctrl+C</kbd> lê o que houver),
`--json`. Veja [Energia](energy.md).

### `emulabzip files <app> [path]`

Lista uma pasta do app. `--root` escolhe a raiz de armazenamento (`data` ou `external` no Android; `data`, `bundle` ou
`group:<id>` no iOS); por padrão a primeira legível é usada. Veja [Storage](storage.md).

### `emulabzip pull <app> <path>`

Copia um arquivo para este computador. `-o` escolhe o destino.

### `emulabzip db <app> <path>`

Abre um banco SQLite em modo somente leitura. Sem opções, lista tabelas e views; `--table` imprime linhas (`--limit`,
`--offset`); `-q, --query` executa uma instrução somente leitura. Todos aceitam `--json`.

### `emulabzip net`

Executa o proxy, aponta o dispositivo para ele e imprime cada requisição quando ela termina, até <kbd>Ctrl+C</kbd>. O
dispositivo é restaurado quando ele para, inclusive após um erro. Veja [Rede](network.md).

| Opção | Significado |
|--------|---------|
| `-p, --port <n>` | Porta em `127.0.0.1` (padrão 9090, ou `EMULABZIP_PORT`). |
| `--no-connect` | Só executa o proxy; não muda nenhum dispositivo. |
| `--har <file>` | Salva as requisições como um arquivo HAR ao parar. |
| `--json` | Um objeto JSON por requisição. |
| `--rules <file>` | Simula, atrasa, bloqueia, mapeia para um arquivo ou outro servidor, verifica contra o OpenAPI ou aplica script nas requisições com as regras de um arquivo JSON. Veja [Mocks e breakpoints](mocks.md) e [Regras de rede](network-rules.md). |
| `--http3` | Fala HTTP/3 com servidores que o oferecem. `--show-h3` também mostra os apps que as ofertas. |
| `--conditions <preset>` | Deixa a rede lenta: `fast-3g`, `slow-3g`, `slow-wifi`, `high-latency`, `packet-loss` ou `offline`. Veja [Condições de rede](network-conditions.md). |
| `--down <kbps>`, `--up <kbps>`, `--latency <ms>`, `--loss <percent>` | Números seus, ou mudanças em um preset. |

Operações GraphQL e métodos e status gRPC são impressos depois de cada requisição.

`emulabzip net import <file> [--har <file>] [--json] [--dns]` lê uma captura `.pcap` ou `.pcapng` e imprime as requisições
que ela contém, com as consultas DNS quando pedido. Veja [Importando uma captura](pcap.md).

### `emulabzip layout` e `emulabzip a11y`

`layout` imprime as views da tela como uma árvore (`--json` para tudo, `--png <file>` para salvar o screenshot tirado junto,
`--depth <n>`, `--app <id>` para marcar uma tela iOS com o framework do seu app). `a11y` audita a tela e imprime os
resultados; `--fail-on note|warning|error` sai com 1 quando algo pelo menos tão grave é encontrado. Veja
[Layout e acessibilidade](layout.md).

### `emulabzip frameworks <app>`

Lista os frameworks de UI com os quais o app é construído (Flutter, React Native, Hermes, Jetpack Compose, SwiftUI) e o que
os revelou. `--json`. Veja [Por dentro dos frameworks](frameworks.md).

### `emulabzip flutter tree|frames|memory <app>`

Olha dentro de um app Flutter em debug ou profile pelo serviço da VM Dart. `--uri <address>` informa onde ele escuta quando
não está no log; `tree` tem `--depth <n>`, `frames` tem `-s, --seconds <n>` (padrão 10), `memory` tem `--top <n>`. Todos
aceitam `--json`.

### `emulabzip rn tree|heap|console|state <app>`

Olha dentro de um app de desenvolvimento React Native pelo Metro (`--metro <port>`, padrão 8081). `tree` tem `--depth <n>`,
`console` tem `-s, --seconds <n>`. `state` lê Redux, React Query, contextos e stores registrados; `--watch` acompanha as ações
do Redux e imprime o que mudou. Veja [Estado do React Native](react-native-state.md). Todos aceitam `--json`. O Metro permite
que um debugger se conecte por vez.

### `emulabzip state save|restore|show|diff`

`save <app> -o file.ezstate [--external]` grava os dados de um app em um arquivo. `restore <file> --yes` substitui os dados do
app por ele, depois de salvar o que está sendo substituído. `show <file>` diz o que um arquivo contém. `diff <a> [b]` lista as
tabelas, colunas, índices e chaves que mudaram entre dois arquivos, ou entre um e o app agora. Veja
[Estado do app](app-state.md).

### `emulabzip edit`

Muda dados no dispositivo: `cell` (`-t <table>`, `--rowid <n>` ou `--pk col=value`, `--set`, `--text`, `--null`), `insert`,
`delete`, `pref-set <key> <value> [--as type]`, `pref-remove`, `plist-set <key/path> <value> [--as type]`, `plist-remove`
e `undo <folder>`. Cada um recebe o app e o arquivo, e **todos exigem `--yes`**. Veja
[Alterando os dados de um app](storage-editing.md).

### `emulabzip avd`, `sim` e `snapshot`

`avd` e `sim` têm `list`, `options`, `create <name> --image <id> --model <id>`, `boot <device>` (`--headless`, `--cold`,
`--wipe`), `shutdown`, `wipe` (`erase` no `sim`) e `delete`. `snapshot` tem `list`, `save`, `load` e `delete` para o emulador
em execução escolhido com `-d`. `wipe`, `delete`, `boot --wipe`, `snapshot load` e `snapshot delete` exigem `--yes`.
`emulabzip devices --all` também lista dispositivos que não estão em execução. Veja [Dispositivos](devices.md).

### `emulabzip audit security <app>`

Procura segredos e dados pessoais no log, nas preferências e nos bancos de dados, HTTP puro e segredos em requisições
(`--har`), as flags `debuggable` e `allowBackup`, componentes que outros apps podem alcançar, exceções do App Transport
Security e lacunas no privacy manifest. `--only manifest,logs,storage,traffic`, `--json`, `--fail-on <note|warning|error>`. A
evidência é cortada para que um segredo nunca seja repetido. Veja [Auditoria de segurança](security-audit.md).

### `emulabzip size show|diff`

`show <file>` detalha um `.apk`, `.aab` ou `.ipa` por tipo, ABI nativa e arquivo dex (`--top`, `--json`); com `--budget` ele
confronta o build com a seção `[size]` do `emulabzip.toml` (`--config`, `--baseline`) e sai com 1 se passar de um limite.
`diff <before> <after>` diz o que mudou (`--fail-on-growth <percent>`). Veja [Tamanho do build](package-size.md).

### `emulabzip i18n check|sweep`

`check` procura texto cortado, fora da tela, truncado ou não espelhado na tela como ela está (`--rtl`, `--fail-on`). `sweep`
testa em outros idiomas e tamanhos de texto (`--locales en-XA,ar-XB`, `--scales 100,130,200`, `--app`, `--fail-on-worse`) e
restaura o dispositivo. Veja [Idioma, tamanho do texto e texto que não cabe](localization.md).

### `emulabzip control locale|reset-locale|font-scale|display|reset-display`

Mudam o idioma de um app (Android) ou do simulador (iOS), o tamanho do texto e o tamanho da tela do Android.
`a11y --simulate <kind> [-o file]` mostra a tela como alguém com daltonismo a vê.

### `emulabzip visual capture|update|compare|list|diff|session`

Baselines da tela, e o que mudou desde então. `capture` e `compare` aceitam `--status-bar` (uma barra limpa 9:41), `--dir`,
`--threshold`; `update` exige `--yes`. `compare` sai com 1 acima do limite e desenha a diferença. Veja
[Regressão visual](visual-regression.md).

### `emulabzip analytics`

Executa o proxy e imprime os eventos que um app envia para Segment, Amplitude, Mixpanel e Firebase (`--service`, `--port`,
`--no-connect`, `--json`), ou, com `--firebase-log`, as linhas de debug do Firebase no log do dispositivo. Veja
[Inspetor de analytics](analytics.md).

### `emulabzip session` e `emulabzip ci`

`session record -o run.ezsession [--app <id>] [--network] [--screens <s>] [--duration <s>] [--video [<s>]] [--taps]
[--replay <file> [--passthrough] [--timing]]` grava até <kbd>Ctrl+C</kbd>; `--video` grava a tela em pedaços (30 s por padrão,
de 1 a 180), `--taps` grava os toques, e `--replay` responde às requisições do app a partir de uma gravação enquanto isso
(exige `--network`). `session show <file> [--json | --markdown]` resume uma sessão ou a pasta `.partial` de uma que foi
interrompida. `session timeline <file> [--lane <name>…] [--json]` imprime toques, requisições, problemas de log, quadros
lentos, garbage collections, crashes, screenshots e vídeo em um único eixo de tempo, e `session video <file> [-o
<dir>]` grava os pedaços da gravação de tela. `ci -o run.ezsession [--app <id>] [--network] [--screens <s>]
[--fail-on-crash] [--summary <file>] [--video [<s>]] [--taps] [--replay <file> [--fail-on-miss]] -- <command…>` grava em
volta de um comando e sai com o código dele. Veja [Sessões](sessions.md), [Linha do tempo e vídeo da tela](timeline.md),
[Reproduzindo uma gravação](replay.md) e [Modo CI](ci.md).

### `emulabzip compare <a> <b>`

Lista o que difere entre dois dispositivos (`--all` para o que também confere, `--json`). Veja
[Comparando dispositivos](compare.md).

### `emulabzip crashes` e `emulabzip crash <id>`

`crashes` lista o que o dispositivo registrou, do mais novo para o mais antigo, com um id para cada; `-a, --app` e `-k, --kind
crash|native|anr` restringem a lista. `crash` mostra um com seu stack e dicas: `--all-threads` imprime todas as threads,
`--raw` imprime o relatório como o dispositivo o escreveu, e `-m, --mapping <file>` o lê com um `mapping.txt`, um source map
ou um `.dSYM` que você indicar, além dos guardados para o app e a versão dele. Ambos aceitam `--json`. Veja
[Crashes](crashes.md) e [Stacks legíveis](symbolication.md).

### `emulabzip mappings` e `emulabzip symbolicate`

`mappings add <app> <version> <file>` guarda um `mapping.txt`, um source map ou um `.dSYM` para um build (`-k, --kind
mapping|sourcemap|dsym` quando não dá para saber pelo arquivo), `mappings list [app] [--json]` mostra o que está guardado, e
`mappings remove <app> <version> [-k <kind>]` o esquece. `symbolicate <stack.txt|-> -m <file>` lê um stack que você colou, de
um arquivo ou da entrada padrão. Veja [Stacks legíveis](symbolication.md).

### `emulabzip report`

Grava o que um relatório de bug precisa em um único arquivo HTML: `[--session <file>] [-a, --app <id>] [-o <file>] [--title <text>]
[--description <text>] [--without screenshot|logs|requests|crashes|video] [--issue github:owner/repo | gitlab:[host/]group/project |
jira:site/PROJECTID/ISSUETYPEID] [--open]`. Com `--issue` ele imprime um link que abre a issue com um resumo preenchido, e
`--open` a abre no navegador. Veja [Relatórios de bug](bug-reports.md).

### `emulabzip control`

Muda o dispositivo. Veja [Controls](controls.md).

| Comando | O que faz |
|---------|--------------|
| `control info [-a <app>]` | O que o dispositivo suporta, sua aparência e as permissões do app. |
| `control location set <lat> <lon>` / `location clear` | Define ou restaura a posição. |
| `control appearance light\|dark` | Modo claro ou escuro. |
| `control statusbar set [--time 9:41] [--battery 100] [--wifi true] [--cellular true]` / `statusbar clear` | Uma barra de status limpa, ou a real de volta. |
| `control battery set <level> [--charging]` / `battery reset` | Finge que a bateria está em um nível (Android). |
| `control permission <app> <permission> grant\|revoke\|reset` | Um nome de permissão do Android, ou um serviço do iOS como `photos`. |
| `control notify [<app>] [--title …] [--body …] [--payload <file>]` | Uma notificação (Android) ou um push simulado (iOS). |
| `control open <url>` | Abre um link no dispositivo. |
| `control tap <x> <y>` / `control swipe <x1> <y1> <x2> <y2> [--ms n]` | Um toque ou um arrasto, em pixels (Android). Veja [Entrada e fluxos](automation.md). |
| `control text <text>` / `control key <name>` | Digita ASCII imprimível, ou pressiona uma tecla (Android). |
| `control launch <app>` | Inicia um app. |

### `emulabzip webview`

As páginas dentro de uma WebView de um app, ou no Chrome, em um dispositivo Android. Veja [Páginas web em apps](webviews.md).

| Comando | O que faz |
|---------|--------------|
| `webview list [--json]` | Os sockets que podem ser inspecionados, e suas páginas. |
| `webview console [--seconds n]` | O que uma página escreve no console, e os erros que ela não captura. |
| `webview eval <expression>` | Executa JavaScript em uma página e imprime o valor. Falha quando o script lança uma exceção. |
| `webview network [--seconds n] [--reload] [--har <file>]` | As requisições que a página faz, como o navegador as vê. |

Cada um aceita `--app <package>`, `--socket <name>` e `--page <id or part of its address>` para dizer qual página, e `--json`.

### `emulabzip flow`

Grava toques em um fluxo, o reproduz e o exporta. Veja [Entrada e fluxos](automation.md).

| Comando | O que faz |
|---------|--------------|
| `flow record --name <name> [--app <app>] [--no-launch] [--seconds n] [--out <file>]` | Grava os toques no dispositivo até <kbd>Ctrl+C</kbd>. |
| `flow list`, `flow show <flow> [--json]`, `flow remove <flow>` | Os fluxos salvos, os passos de um e a exclusão de um. Um fluxo é seu id ou um arquivo `.json`. |
| `flow play <flow>` | O reproduz no dispositivo, escalado para a tela dele. |
| `flow export <flow> --maestro\|--script [--out <file>]` | Um fluxo Maestro, ou um script de shell com comandos `emulabzip control`. |

### `emulabzip link`

Abre um deep link e verifica se o domínio e o app concordam sobre ele. Veja [Deep links](deep-links.md).

| Comando | O que faz |
|---------|--------------|
| `link open <url> [--app <app>]` | O abre no dispositivo e o mantém no histórico. |
| `link check <url> [--app <app>] [--json]` | Lê o `assetlinks.json` ou o `apple-app-site-association` do domínio, pergunta ao dispositivo e lista os resultados com a correção. Sai com erro quando um deles é um problema. |
| `link verify <app>` | Pede ao Android que verifique de novo os domínios do app. |
| `link history [--json]` | Os links abertos e verificados antes. |

### `emulabzip openapi check`

`openapi check --spec <file> --har <file> [--json]` verifica cada resposta de um arquivo HAR contra uma descrição OpenAPI 3, e
sai com erro quando uma não confere. Veja [Regras de rede](network-rules.md#contract).

### `emulabzip script`

Aprova os scripts WebAssembly que as regras executam. `script show <file>`, `script approve <file>`, `script revoke <file or hash>`
e `script list`. Veja [Scripts de regras](rule-scripts.md).

### `emulabzip plugin`

Executa seus próprios coletores. Veja [Plugins](plugins.md).

| Comando | O que faz |
|---------|--------------|
| `plugin list [--json]` | Os plugins instalados e se cada um está ativado. |
| `plugin new <name>` | Começa um plugin a partir de um template. |
| `plugin install <folder> [--force]` | Copia uma pasta de plugin para dentro. Ele começa desativado. |
| `plugin enable <name>` / `plugin disable <name>` | Permite ou deixa de permitir que um plugin execute. |
| `plugin run <name> [-a <app>] [--seconds <n>] [--json]` | Executa um plugin no dispositivo e imprime o que ele informa. |

### `emulabzip mcp [--allow-writes [--no-confirm]]`

Serve os dispositivos a um assistente de IA por MCP na entrada e saída padrão. Veja [Servidor MCP](mcp.md) e a
[referência do MCP](mcp-tools.md).

| Flag | Faz |
|------|------|
| `--allow-writes` | Também oferece as ferramentas que apagam, excluem, restauram ou reescrevem dados. Cada chamada pede sua confirmação antes |
| `--no-confirm` | Com `--allow-writes`, pula a pergunta. Só para uso sem supervisão em um dispositivo descartável |

### `emulabzip trace`

Grava e analisa traces. Veja [Traces](traces.md). <kbd>Ctrl+C</kbd> durante uma gravação a interrompe mais cedo e mantém o que
foi gravado.

| Comando | O que faz |
|---------|--------------|
| `trace record` | Grava um trace de sistema do Perfetto (Android 9+). `-p, --preset cpu\|frames\|memory\|custom`, `--config <file>` para `custom`, `-D, --duration <s>` (padrão 10), `-a, --app <pkg>`, `-o, --output <file>`, `--open` para mostrá-lo na UI do Perfetto. |
| `trace cpu` | Amostra um app (`-a, --app`) ou o sistema inteiro (`--system`) com o simpleperf e imprime as funções quentes. `-o` salva o relatório, `--folded <file>` os stacks agrupados, `--top <n>` o tamanho da lista. |
| `trace instruments` | Grava um template do Instruments de um simulador (`-t, --template`, padrão Time Profiler) em um documento `.trace`. Exige o Developer Mode. |
| `trace summary <file>` | CPU, quadros, memória e os slices mais ocupados de um trace do Perfetto. `--json` para scripts. |
| `trace query <file> "<sql>"` | Executa PerfettoSQL em um trace do Perfetto. `--json` para scripts. |
| `trace open <file>` | Abre um trace do Perfetto na UI do Perfetto. |
| `trace list` | Lista as gravações feitas no app desktop. |

`summary` e `query` baixam o trace processor na primeira vez (veja [Traces](traces.md#the-trace-processor)).

### `emulabzip ca install`

Faz o dispositivo confiar no certificado: o repositório do sistema em um emulador Android com acesso root, o keychain em um
simulador iOS, e, para outros dispositivos Android, copia o arquivo e imprime os passos. `emulabzip ca path` imprime onde está
o arquivo do certificado.

## Ambiente

| Variável | Efeito |
|----------|--------|
| `EMULABZIP_DEVICE` | Padrão para `--device`. |
| `EMULABZIP_LOG=debug` | Diagnósticos detalhados no stderr. |
| `EMULABZIP_HOME` | Pasta da autoridade certificadora, das configurações de proxy salvas, das gravações e do trace processor. |
| `EMULABZIP_PORT` | Padrão para `net --port`. |
| `ADB_SERVER_SOCKET`, `ANDROID_ADB_SERVER_PORT`, `EMULABZIP_ADB` | Veja [Android](android.md). |
