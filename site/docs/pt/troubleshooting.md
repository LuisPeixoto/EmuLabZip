---
source: a63f6e8c9dee
description: "Soluções para os problemas mais comuns do EmuLabZip: dispositivos que não aparecem, logs vazios, falhas de rede, HTTPS, crashes, traces, vídeo, plugins e MCP."
---
# Solução de problemas

## Meu emulador Android não aparece

1. Execute `adb devices`. Se o emulador não estiver listado ali, o EmuLabZip também não consegue vê-lo. Reinicie-o ou
   execute `adb kill-server && adb start-server`.
2. O rodapé do device picker mostra **adb unavailable** quando o `adb` não é encontrado. Instale as platform tools do
   Android SDK, ou aponte `ANDROID_HOME` ou `EMULABZIP_ADB` para o seu SDK.
3. Um dispositivo listado como **unauthorized** está esperando que você aceite o aviso de depuração por USB.
4. Uma porta personalizada do servidor ADB precisa ser definida com `ANDROID_ADB_SERVER_PORT` ou `ADB_SERVER_SOCKET`
   também para o EmuLabZip.

## Meu simulador não aparece

- Apenas simuladores **iOS inicializados** são listados. Confira com `xcrun simctl list devices booted`.
- O rodapé mostra **simctl unavailable** quando as ferramentas de linha de comando do Xcode estão ausentes. Execute
  `xcode-select --install`, ou `sudo xcode-select -s /Applications/Xcode.app` se houver várias versões do Xcode
  instaladas.

## O log está vazio

- No Android, confira se o `adb logcat` imprime linhas.
- Olhe o nível mínimo e os filtros; **Reset filters** aparece quando existem linhas, mas nenhuma corresponde.
- O filtro de app precisa que o app esteja em execução. Inicie o app; o filtro pega o novo id de processo
  automaticamente.

## O log está movimentado demais

Veja [O log unificado é movimentado](ios-simulator.md#the-unified-log-is-busy). No Android, aumente o nível mínimo ou
filtre por tag.

## Os screenshots falham

- No Android, `adb exec-out screencap -p > shot.png` precisa funcionar. Algumas telas seguras (vídeo com DRM, apps de
  banco) são pretas por design.
- No iOS, `xcrun simctl io booted screenshot shot.png` precisa funcionar.

## O Storage diz "only debuggable apps can be browsed"

O Android só deixa o `run-as` ler os dados privados de apps depuráveis. Instale um build de debug do seu app, ou teste
as ferramentas com o app de exemplo (`tools/sample-app/install.sh`). O armazenamento compartilhado funciona para todos
os apps.

## Os heap dumps falham no Android

O `am dumpheap` precisa de um app depurável, ou de uma imagem de emulador do tipo `userdebug`/`eng` (imagens sem Google
Play). O erro mostra o motivo que o Android deu, por exemplo `Process not debuggable`.

## O Performance não mostra a taxa de quadros

Os quadros só são contados enquanto o app desenha. Interaja com o app; uma tela estática não renderiza nada. No iOS
Simulator, as métricas de quadros chegam com os traces do Instruments na Phase 4.

## Network: nada aparece

- Confira se o dispositivo diz *sending traffic here*. No Android, `adb shell settings get global http_proxy` deve
  imprimir `127.0.0.1:<port>`.
- Alguns apps ignoram o proxy do sistema (os que abrem sockets por conta própria, ou usam HTTP/3). Eles não podem ser
  capturados.
- Apps que usam muito cache podem nem consultar a rede. Limpe os dados deles ou use uma URL nova.

## Network: as requisições HTTPS falham ou aparecem como "Connections"

O dispositivo não confia no certificado, ou o app fixa o do seu servidor (pinning). Pressione **Certificate** e siga o
que ele diz. Nas imagens Android do Google Play, apenas o Chrome e os apps que permitem certificados de usuário vão
confiar nele; veja [o certificado](network.md#the-certificate). Um banner nomeia os hosts que estão sendo repassados sem
decifrar; use **Decrypt again** depois de corrigir.

## Network: o dispositivo ou o Mac continua apontando para o proxy

Pressione **Disconnect**, ou execute `adb shell settings put global http_proxy :0` no Android. No Mac, abra o EmuLabZip
uma vez: ele restaura as configurações de proxy que salvou se uma execução anterior foi encerrada à força. Caso
contrário, desative os proxies em *System Settings › Network › Details › Proxies*.

## Network: o macOS pede uma senha

Alterar o proxy do Mac é uma configuração do sistema. O EmuLabZip tenta como você mesmo e só pede direitos de
administrador se o macOS recusar.

## Network: uma regra não faz nada

As regras veem requisições decifradas, então o HTTPS precisa do certificado instalado, e um host que o rejeitou é
repassado sem decifrar e não pode ser alterado. A primeira regra ativada que corresponde vence; mova a que você espera
para cima das outras. Um padrão de host corresponde apenas ao host (`api.example.com`, não
`https://api.example.com/`).

## Network: o HTTP/3 não mostra `h3`

O recurso vem desativado por padrão (**HTTP/3 → Use HTTP/3 to servers**). Um host só é tentado depois que uma de suas
respostas disse que oferece HTTP/3, então a primeira requisição é sempre TCP. Uma rede que bloqueia UDP faz o QUIC
falhar; o host então fica no TCP por 10 minutos.

## Layout: nada para ler no iOS

O Simulator.app precisa estar aberto naquele dispositivo, e o programa que executa o EmuLabZip precisa ter permissão em
System Settings, Privacy & Security, Accessibility (o app, ou o seu terminal para `emulabzip layout`). Acorde o
dispositivo se ele mostrar uma tela preta.

## Flutter: "Could not find where the Dart VM service listens"

Apenas os builds de debug e profile têm um, e o app grava o endereço dele no log uma única vez, quando inicia. Inicie o
app de novo, ou informe o endereço da saída do `flutter run` com `--uri`. Um build de release não tem VM service.

## Flutter: a árvore não tem nomes de arquivo

O app não foi compilado com `--track-widget-creation`. O `flutter run` o adiciona; para `flutter build`, adicione
`--extra-front-end-options=--track-widget-creation`.

## React Native: "Metro is not running" ou "does not list a runtime"

Inicie o Metro (`npx react-native start`) e abra o app em um dispositivo que o alcance (`adb reverse tcp:8081
tcp:8081` no Android). O Metro só lista um runtime enquanto o app está conectado a ele, e deixa entrar um depurador por
vez: feche o React Native DevTools e o inspetor do Chrome e tente de novo. Os builds de release não têm Metro nem o
hook do React DevTools.

## State: "RN state is empty" ou "no DevTools hook"

Redux, React Query e contextos são encontrados pelo hook do DevTools do React, que só os builds de desenvolvimento têm.
Em um build de release, registre os stores em `globalThis.__EMULABZIP_STORES__` (veja
[Estado do React Native](react-native-state.md)). Um store usado sem um `Provider` do react-redux também precisa ser
registrado.

## State: restaurar ou salvar falha no Android

O app precisa ser depurável (como no [Storage](storage.md)). O `run-as` e o `tar` rodam no dispositivo: uma imagem de
emulador com o `run-as` bloqueado não consegue salvar um estado. Uma restauração que diz que o tar foi recusado achou
uma entrada fora da pasta do app; o arquivo não é do EmuLabZip, ou está danificado.

## Edit: "the table cannot be changed"

Uma linha é encontrada por `rowid` ou pela sua chave primária. Uma view, ou uma tabela sem nenhum dos dois, não tem como
distinguir suas linhas: altere-a com SQL no app. Uma edição que é lida de volta de forma diferente restaura o arquivo
antigo e diz isso.

## Devices: um emulador não inicia

O `emulabzip avd boot` precisa do `emulator` do SDK (encontrado a partir de `ANDROID_HOME`, `ANDROID_SDK_ROOT` e das
pastas usuais). Execute `emulator -avd <name>` você mesmo para ver a mensagem dele. Um emulador em execução nunca é
apagado nem excluído: desligue-o antes.

## Audit: "the area could not be read"

O log precisa que o app esteja **em execução**, para que as linhas dele possam ser distinguidas das do sistema: inicie-o
e use-o, depois audite de novo. O Storage no Android precisa de um app depurável, como no [Storage](storage.md). As
requisições vêm do proxy (inicie-o na view Network, ou passe `--har`).

## Language: "Android changes the language of one app at a time"

O Android não tem como mudar o idioma de um dispositivo pelo shell, apenas o de um app (`cmd locale set-app-locales`,
Android 13 e mais recentes). Passe `--app`, ou escolha o app na barra de ferramentas. O iOS Simulator muda o idioma de
todos os apps, e cada um o assume quando inicia de novo.

## Text checks: um label é reportado sem estar cortado

O `text-cut-off` é uma heurística: uma única linha cuja caixa foi limitada ao espaço que o seu contêiner (ou a próxima
view na sua linha) deixou, e cujo texto vai até o fim da caixa. Confira a 130% e a 200%; se couber, ignore.

## Visual: "the screenshots are not the same size"

Eles vêm de dispositivos, orientações ou tamanhos de tela diferentes. Compare coisas iguais: o mesmo dispositivo, a mesma
orientação e o mesmo tamanho de tela (`control reset-display` devolve o de um Android ao normal).

## Network: um build do Gradle não consegue baixar sua distribuição

Não é assunto do EmuLabZip, mas aparece com os apps de demonstração: algumas redes deixam o `curl` alcançar os servidores
do Gradle enquanto o Java dá timeout. Baixe a distribuição com o `curl` e coloque-a onde o Gradle espera
(`~/.gradle/wrapper/dists/<name>/<hash>/`), ou compile com um Gradle que você já tenha.

## Layout: o Android diz que o uiautomator não produziu nada

Algumas janelas o proíbem: telas seguras e alguns players. Vá para outra tela e tente de novo. Se o dispositivo estiver
ocupado (uma animação de boot, uma caixa de diálogo de ANR), espere um momento.

## Sessions: uma gravação ficou como uma pasta `.partial`

A gravação foi encerrada à força antes de conseguir se empacotar. Nada se perde: `emulabzip session show
run.ezsession.partial` a abre. Exclua a pasta, ou mova-a, antes de gravar de novo com o mesmo nome.

## Timeline: faltam toques

Os toques são lidos do dispositivo de entrada do kernel com `getevent`, então um dedo na janela do emulador ou um evento
enviado pelo console do emulador é visto. O `adb shell input tap` é injetado mais acima, dentro do Android, e não é. O
iOS não entrega os toques do simulador a nada fora dele, então ali não há nenhum. Grave com `--taps`.

## Timeline: os horários do log são "estimated"

A gravação foi feita antes do EmuLabZip 0.10, ou o dispositivo não soube informar a hora, então os horários locais do
log foram posicionados comparando a primeira linha com o início da gravação. Eles podem estar errados pela diferença
entre os dois relógios. Grave de novo para ter horários exatos.

## Video: "screenrecord made no video"

Algumas telas não podem ser gravadas (janelas seguras) e algumas imagens não oferecem suporte ao `screenrecord`. A
mensagem traz o que a ferramenta imprimiu. No iOS Simulator, o `recordVideo` precisa que o simulador esteja
inicializado; se não começar em 20 segundos, a mensagem diz o motivo.

## Video: o relatório tem um `.mp4` separado

Um clipe de até 20 MB faz parte do arquivo HTML. Um maior é gravado ao lado dele como `emulabzip-report.mp4`, e o
arquivo se refere a ele por esse nome: mova ou anexe os dois juntos.

## Replay: "not in the recording"

O app pediu algo que a gravação não tem: outro endereço, outra query, um método que não foi usado, ou uma requisição que
falhou quando foi gravada. O proxy responde 504 e a lista em **Not recorded** (ou no fim do `emulabzip net --replay`).
Grave de novo com essa requisição, ou adicione `--passthrough` para deixá-la chegar ao servidor. A correspondência
ignora a ordem dos parâmetros de query e a porta, mas não os valores: um app que adiciona um timestamp ou um id
aleatório a toda requisição sempre vai falhar.

## Replay: WebSockets e algumas conexões falham

Enquanto uma gravação é reproduzida, o que não pode ser respondido a partir de um arquivo HAR é recusado, para que não
alcance a internet: WebSockets, chamadas gRPC, apps que não aceitam o certificado do EmuLabZip e conexões sem TLS que o
proxy não consegue ler. Use `--passthrough` se elas precisarem funcionar.

## Traces: "Perfetto did not record a trace"

O Perfetto precisa do Android 9 ou mais recente. Se a mensagem citar uma permissão, reinicie o emulador, ou execute
`adb shell setprop persist.traced.enable 1` e grave de novo. Uma configuração personalizada com erro de sintaxe é
reportada pelo próprio Perfetto; confira-a com
`adb shell perfetto --txt -c - -o /data/misc/perfetto-traces/test <config.txt`.

## Traces: um perfil de CPU falha

O simpleperf só amostra apps que são *debuggable* ou *profileable*, e o app precisa estar em execução. Para o sistema
inteiro, você precisa de uma imagem de emulador que permita `adb root` (não Google Play). "No samples were recorded"
significa que o app não fez nada durante a gravação: use-o e grave de novo.

## Traces: o Instruments diz que o Developer Mode está desativado

Execute `sudo DevToolsSecurity -enable` em um terminal, digite sua senha e grave de novo. `DevToolsSecurity -status`
mostra o estado atual.

## Traces: o download do trace processor falha

Ele vem de `github.com/google/perfetto/releases`. Atrás de um proxy, defina `HTTPS_PROXY`. Um arquivo que não
corresponde ao SHA-256 fixado é descartado e nunca executado. Você ainda pode abrir qualquer trace do Perfetto na UI do
Perfetto.

## Crashes: nada é listado

No Android, o sistema guarda um crash no DropBox e limita a taxa de repetições, então um app em loop é listado menos
vezes do que quebra. No iOS Simulator, o macOS grava o relatório `.ips` alguns segundos depois do crash, em
`~/Library/Logs/DiagnosticReports`; relatórios de apps que não estão na pasta de um simulador não são listados.
Pressione **Refresh**, ou execute `emulabzip crashes`.

## Crashes: os nomes continuam embaralhados

Um mapping só é usado para a versão do app para a qual foi gerado, e o crash diz qual é ("Add the mapping.txt of
com.example 1.2.0 (42)"). Adicione o arquivo exatamente com essa versão: `emulabzip mappings add com.example
"1.2.0 (42)" mapping.txt`. Um arquivo de outro build daria nomes errados, então não é usado. Os frames também ficam como
estão quando o R8 não tinha números de linha para manter: adicione `-keepattributes SourceFile,LineNumberTable` às
regras do próximo build.

## Crashes: o React Native mostra `1:608617` em vez de um arquivo

Um build de release executa um bundle, e as posições dele precisam do source map desse bundle. Para o Hermes, use o mapa
composto, `android/app/build/generated/sourcemaps/react/release/index.android.bundle.map`, e não os `packager` ou
`compiler`. Ele também precisa ser o mapa do build que quebrou.

## Crashes: o iOS mostra endereços puros

Adicione o `.dSYM` do build que quebrou (`emulabzip mappings add com.example.app "1.2.0 (42)" MyApp.app.dSYM`). O UUID
dele precisa corresponder ao do app, então um dSYM de outro build, mesmo da mesma versão, não é usado. Ler os endereços
exige um Mac, porque usa o `atos`.

## Bug report: nenhuma requisição com falha

As requisições vêm do proxy em Network. Inicie-o e conecte o dispositivo antes de o bug acontecer; um relatório feito a
partir de uma [sessão](sessions.md) gravada com `--network` também as tem.

## Controls: a localização não faz nada em um dispositivo

Apenas emuladores aceitam uma nova localização, pelo console do emulador. Isso exige o token em
`~/.emulator_console_auth_token`, que o emulador cria na primeira vez que roda a partir da sua conta. Dispositivos
físicos e simuladores de outros tipos não têm suporte para localização no Android.

## Controls: uma notificação não aparece no Android

As notificações são publicadas pelo shell com `cmd notification post` (testado no Android 14). Ative as notificações
para o "Android Shell" se o dispositivo as esconder. O argumento de app é ignorado no Android.

## Web pages: minha WebView não está listada

Uma WebView só é listada quando o seu app chama `WebView.setWebContentsDebuggingEnabled(true)` (um build de debug
geralmente chama) e a página está aberta. O Chrome é listado enquanto está aberto. A lista é lida quando você abre
**Frameworks** e quando pressiona **Refresh**. No iOS, use o menu Develop do Safari; veja
[Páginas web nos apps](webviews.md).

## Flows: meus toques não são gravados

Apenas a tela de toque é lida: toque na janela do próprio emulador. Os toques enviados pelo EmuLabZip (**Interact**,
**Play**) ou por `adb shell input` nunca chegam a ela, e o texto digitado não é gravado. Um toque recebe o nome da view
que atingiu apenas quando a tela foi lida a tempo, o que leva dois ou três segundos antes do toque; o ponto é guardado de
qualquer forma. Veja [Entrada e fluxos](automation.md).

## Deep links: a verificação diz que o domínio não está verificado

No Android, peça ao dispositivo para tentar de novo com **Verify again** ou `emulabzip link verify <app>`, e confira
depois de alguns segundos. Uma divergência de fingerprint geralmente significa que o arquivo lista a chave de release e o
build no dispositivo está assinado com a chave de debug. Um arquivo de domínio que redireciona (para `www`, ou de http
para https) é recusado pelos dois sistemas. Veja [Deep links](deep-links.md).

## Network: uma regra de script não faz nada

Um script só roda depois que os bytes exatos dele foram aprovados (**Approve this script** na regra, ou
`emulabzip script approve`). Altere o arquivo e ele precisa ser aprovado de novo. Se um script roda mas a requisição
continua igual, os detalhes do fluxo dizem o que o script escreveu ou por que falhou. Veja
[Scripts de regra](rule-scripts.md).

## Plugins: um plugin não pode ser executado

Ele precisa estar ativado antes, e editar o `plugin.json` dele o desativa de novo. Um plugin que termina com erro mostra
o fim da sua saída de erro. Confira se o programa em `command` existe (ele é procurado na pasta do plugin e depois no
PATH) e se é executável (`chmod +x run.sh`).

## MCP: o assistente não vê nenhuma ferramenta

Execute `emulabzip mcp` à mão e envie a ele uma requisição `initialize`, como no [servidor MCP](mcp.md#trying-it). Se o
cliente não encontrar o `emulabzip`, dê a ele o caminho completo. A saída padrão carrega apenas o protocolo; olhe a
saída de erro padrão (`EMULABZIP_LOG=debug`) para ver o que deu errado.

## Obtendo diagnósticos

Inicie o app ou a CLI com `EMULABZIP_LOG=debug` e inclua a saída ao
[abrir uma issue](https://github.com/LuisPeixoto/EmuLabZip/issues/new?template=bug_report.yml).
