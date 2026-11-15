---
source: c99e4d2c05ff
description: "Inspecione as páginas web dentro de apps híbridos: console, JavaScript e requisições da WebView, como no DevTools do Chrome, e Safari no iOS."
---
# Páginas web em apps

Apps híbridos mostram páginas web em uma **WebView**. O EmuLabZip as lê como o DevTools do Chrome: lista as páginas, mostra
o console delas, executa JavaScript nelas e mostra as requisições que a página faz, na visão Network, junto com tudo o que o
proxy viu. No iOS, ele leva você ao Web Inspector do Safari.

## O que um app precisa permitir

No Android, uma WebView só pode ser inspecionada quando o app liga a depuração web:

```kotlin
WebView.setWebContentsDebuggingEnabled(true)   // in a debug build
```

O Chrome no dispositivo também é listado, enquanto estiver aberto. Não há nada para instalar nem porta para encaminhar: o
EmuLabZip lê a lista de sockets abertos do dispositivo, encontra `webview_devtools_remote_<pid>` e `chrome_devtools_remote`
e conversa com eles pelo ADB.

`tools/compose-app` tem uma tela para testar: **Open the web page** abre uma WebView com uma página que escreve no console,
busca dados do site, busca algo que não existe e lança um erro.

## No app

**Frameworks → Web pages** lista todas as páginas que podem ser inspecionadas, com o app a que pertencem. Escolha uma e:

- o **console** acompanha ao vivo: logs, avisos, erros e os erros que a página não captura, cada um com a sua origem;
- a linha na parte de baixo **executa JavaScript** na página, como `document.title`, e mostra o valor;
- **Reload** recarrega a página, ignorando o cache;
- **Show requests in Network** também lista as requisições da página na visão [Network](network.md), marcadas com **WEBVIEW**.
  Escolha a página de novo depois de mudá-la.

As requisições vêm da própria página, então aparecem mesmo quando o proxy não conseguiu decifrar o tráfego (o certificate
pinning não importa para a página) e sem instalar um certificado. Elas não podem ser alteradas nem seguradas em um
breakpoint como as requisições do proxy; para isso, use o proxy.

![O painel Web pages com o console da WebView da demo](images/webview-console.png)

![As requisições da WebView em Network, marcadas com WEBVIEW](images/webview-network.png)

## No iOS

A Apple não deixa outras ferramentas lerem uma `WKWebView`, então em um simulador o painel mostra os passos e um botão que abre o Safari:

![O painel Web pages em um iOS Simulator](images/webview-ios.png)

1. No Safari, abra Settings → Advanced e ative "Show features for web developers".
2. Abra a página no seu app. Uma `WKWebView` precisa ter `isInspectable = true` (iOS 16.4 e posterior; as builds de debug geralmente definem isso).
3. No menu **Develop** do Safari, escolha o Simulator e depois a página.

## Na linha de comando

```bash
emulabzip webview list                                   # the sockets and their pages
emulabzip webview console --app com.example.shop         # what the page writes, for 10 seconds (--seconds N)
emulabzip webview eval "document.title"                  # run JavaScript; a script that throws fails the command
emulabzip webview network --reload --seconds 10 --har page.har
```

`--app` escolhe a WebView do app, `--socket` um socket pelo nome, `--page` uma página pelo id ou por parte do endereço. Cada
um pode ser omitido quando só há uma opção. `--json` está disponível em cada comando.

## A partir de um assistente

`webview_list` e `webview_console` são [ferramentas MCP](mcp.md). Executar JavaScript não é oferecido via MCP, porque um
script pode mudar o que o app armazena; isso fica no app e na linha de comando.

## Limites

- Somente Android, e somente WebViews depuráveis e o Chrome. Uma build de release que não ligou a depuração não pode ser lida.
- Os corpos de resposta são lidos até 10 MB. Um corpo que já saiu da memória do navegador quando é pedido aparece sem o texto.
- A página é lida por uma conexão própria. Outro depurador (`chrome://inspect`) pode estar aberto ao mesmo tempo.
