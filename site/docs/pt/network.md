---
source: 0c82c94d90e4
description: A ferramenta Network é um proxy de depuração que decifra HTTPS com uma CA local e lista cada requisição com cabeçalhos, corpo e tempos, sem mudar o app.
---
# Network

A ferramenta Network é um proxy de depuração embutido no EmuLabZip. Ela fica entre o seu app e a internet, decifra o HTTPS
com uma autoridade certificadora que vive só no seu computador e lista cada requisição com seus cabeçalhos, corpo e
tempos. Você não muda o seu app.

<p align="center">
  <img src="images/network.png" alt="A ferramenta Network listando requisições, com uma resposta JSON aberta" />
</p>

## Começo rápido

1. Abra **Network** (<kbd>⌘6</kbd>) e escolha o seu dispositivo.
2. Pressione **Certificate** uma vez por dispositivo, para que ele confie no certificado do EmuLabZip (veja [abaixo](#the-certificate)).
3. Pressione **Connect**. O proxy inicia se não estiver rodando, e o dispositivo envia o tráfego HTTP e HTTPS para ele.
4. Use o seu app. As requisições aparecem conforme terminam. Clique em uma para ver seus cabeçalhos, corpo e tempos.
5. Pressione **Disconnect** (ou simplesmente saia) quando terminar. O EmuLabZip devolve o dispositivo ao estado em que o encontrou.

Em um terminal:

```bash
emulabzip ca install                 # once per device
emulabzip net --har capture.har      # Ctrl+C to stop; the device is restored
```

## Como um dispositivo chega ao proxy

O proxy só escuta em `127.0.0.1` do seu computador, nunca na rede.

| | Android | iOS Simulator |
|---|---------|---------------|
| Rota | `adb reverse tcp:<port> tcp:<port>` faz `127.0.0.1:<port>` no dispositivo chegar ao seu computador, e depois `settings put global http_proxy 127.0.0.1:<port>` | O simulador compartilha a rede do Mac, então o proxy HTTP e HTTPS **do próprio Mac** é apontado para o EmuLabZip |
| Escopo | O único dispositivo que você conecta | **Todos os apps do Mac** enquanto estiver ligado |
| Desfeito por | Disconnect, sair do app, ou o `emulabzip net` parar. Só o encaminhamento que o EmuLabZip criou é removido; o `tcp:8081` do Metro continua | Disconnect, sair do app, ou o próximo início se o EmuLabZip foi encerrado à força |
| Funciona com | Emuladores, dispositivos USB, depuração sem fio | Simuladores no macOS |

No Mac, o EmuLabZip salva as configurações de proxy de cada serviço de rede antes de mudá-las e as grava de volta
exatamente, inclusive um proxy que estava desligado mas ainda tem um endereço. Se o macOS pedir uma senha de administrador
na primeira vez, é o `networksetup` mudando configurações do sistema.

## O certificado

Para ler o HTTPS, o EmuLabZip responde com um certificado do site, assinado pela sua própria autoridade certificadora. O
dispositivo precisa confiar nessa autoridade. Ela é criada no primeiro uso, chamada *EmuLabZip CA*, e guardada em
`~/Library/Application Support/EmuLabZip` (`%APPDATA%\EmuLabZip` no Windows, `~/.local/share/EmuLabZip` no Linux),
com a chave privada legível só por você. Defina `EMULABZIP_HOME` para usar outra pasta.

| Dispositivo | O que **Certificate** faz | Quais apps confiam nele |
|--------|---------------------------|---------------------|
| iOS Simulator | `simctl keychain add-root-cert` | Todos. Reinicie os apps que já estavam rodando |
| Emulador Android cuja imagem permite `adb root` (Google APIs, AOSP) | Adiciona ao repositório do **sistema** | **Todos os apps**, até o dispositivo reiniciar |
| Imagem Android do Google Play, ou um dispositivo físico | Copia `emulabzip-ca.crt` para Downloads e abre Settings para você instalá-lo como certificado de **usuário** | O Chrome e os apps que aderem. Suas próprias builds de debug podem aderir com o trecho abaixo |

Em uma imagem com root, o `adbd` fica rodando como root, como depois de `adb root`. Executar **Certificate** de novo é seguro: ele
substitui o arquivo em vez de empilhar outra cópia.

### Fazendo a sua build de debug confiar nele

Desde o Android 7, os apps ignoram certificados de usuário a menos que peçam por eles. Adicione isto somente à sua build de **debug** e
referencie no manifest dessa build (`android:networkSecurityConfig="@xml/network_security_config"`):

```xml
<?xml version="1.0" encoding="utf-8"?>
<network-security-config>
    <debug-overrides>
        <trust-anchors>
            <certificates src="system" />
            <certificates src="user" />
        </trust-anchors>
    </debug-overrides>
</network-security-config>
```

As builds de release não são afetadas, porque `debug-overrides` só vale para apps depuráveis.

### Sem instalar nada

Com o proxy rodando, um navegador no dispositivo pode buscar o certificado no próprio proxy:
`http://127.0.0.1:9090/ca.crt` (DER, para Android e iOS) ou `/ca.pem`.

## O que você vê

Cada linha é uma requisição. As colunas são o status, método, host, caminho, tipo, tamanho e tempo. Filtre por texto (URL,
método ou status), por classe de status ou por método. Use <kbd>↑</kbd> <kbd>↓</kbd> para se mover pela lista.

O painel de detalhes tem quatro abas:

| Aba | Mostra |
|-----|-------|
| Headers | Cabeçalhos da requisição e da resposta |
| Request | O corpo que foi enviado |
| Response | O corpo que voltou |
| Timing | O protocolo, o tempo até o primeiro byte e o tempo até o fim |

Os corpos são decodificados para leitura (gzip, deflate, Brotli, zstd): você vê o tamanho na rede e o tamanho decodificado.
O JSON é formatado, as imagens são exibidas e dados binários aparecem como um dump hexadecimal. O proxy encaminha os bytes exatamente como
eram; só a cópia que você lê é decodificada.

**Connections** são o que o proxy não conseguiu decodificar: tráfego que não é HTTP e apps que recusam o certificado. Elas são
listadas para você saber que existem, com o motivo. Os WebSockets são listados com uma aba **Messages** que mostra cada mensagem
(veja [Mensagens de WebSocket](websockets.md)).

A coluna **Proto** mostra o que o proxy falou com o servidor: `h1`, `h2` ou `h3`.

## Mais do que observar

- **Rules**: responda uma requisição você mesmo, deixe-a lenta, bloqueie-a ou pare-a e edite-a. Veja
  [Mocks e breakpoints](mocks.md).
- **Import**: abra uma captura `.pcap` ou `.pcapng` como requisições. Veja [Importando uma captura](pcap.md).
- **Send again**: **Resend** envia uma requisição mais uma vez como estava; **Edit and resend** deixa você mudar antes o método,
  o endereço, os cabeçalhos e o corpo. A cópia é uma nova linha na lista marcada com `RESEND`. Regras e breakpoints não
  se aplicam a ela, já que você escolheu enviá-la. Uma requisição simulada por mock, um WebSocket e uma requisição lida de uma captura não podem ser enviados
  de novo, nem uma cujo corpo era grande demais para guardar.
- **GraphQL e gRPC**: as operações são nomeadas na lista, e os corpos são lidos como o protocolo deles exige. Veja
  [GraphQL](graphql.md) e [gRPC e protobuf](grpc.md).
- **Network conditions**: uma conexão lenta, com latência, com perdas ou ausente. Veja [Condições de rede](network-conditions.md).
- **Replay**: responda a partir de um arquivo HAR ou de uma sessão gravada, para o app rodar sem a internet. Veja
  [Reproduzindo uma gravação](replay.md).
- **HTTP/3**: veja abaixo.

![O editor para reenviar uma requisição GraphQL com outra variável](images/network-resend.png)

### Apps que fixam o certificado

Alguns apps (muitos serviços do Google, apps de bancos) aceitam apenas o certificado do próprio servidor. O EmuLabZip não consegue decifrá-los
sem quebrá-los, então depois que um host rejeita o certificado uma vez, as conexões seguintes a ele são **repassadas**
sem mudança. O app continua funcionando, a lista mostra uma conexão com falha com o motivo, e um banner nomeia
os hosts. **Decrypt again** tenta de novo, por exemplo depois de você instalar o certificado.

## Exportar

O botão de download salva as requisições como um arquivo [HAR](http://www.softwareishard.com/blog/har-12-spec/), legível pelo
Chrome DevTools, pelo Firefox e pela maioria das ferramentas de API. Os corpos são guardados decodificados. As conexões ficam de fora. Na linha
de comando: `emulabzip net --har capture.har`.

## HTTP/3

Um dispositivo atrás de um proxy HTTP nunca fala QUIC com ele, então o HTTP/3 só diz respeito ao segundo trecho do proxy, até o servidor.
Os servidores anunciam o HTTP/3 em um cabeçalho `Alt-Svc: h3=":443"`. O proxy o lembra por host, e com **HTTP/3 → Use HTTP/3
to servers** ligado (vem desligado por padrão), as requisições seguintes a esse host vão por QUIC. Se o QUIC falhar (uma rede que
bloqueia UDP, um servidor que não responde), a requisição volta para HTTP/2 ou 1.1 e o host fica no TCP por 10
minutos. Os fluxos mostram `h3` na coluna Proto quando funcionou. Uma resposta HTTP/3 é lida por inteiro antes de ser repassada,
então um download muito grande não é transmitido em streaming.

**Hide HTTP/3 from apps** (ligado por padrão) remove as entradas `h3` do cabeçalho `Alt-Svc` que o app recebe. Sem isso,
um app pode saber que o servidor fala HTTP/3 e tentar QUIC diretamente, contornando o proxy, e o tráfego dele sumiria
da lista. O fluxo ainda registra o cabeçalho original. Na linha de comando: `emulabzip net --http3`, e
`--show-h3` para mostrar as ofertas aos apps.

## Limites

- Só os primeiros **10 MB** de cada corpo são guardados. O resto ainda chega ao destino.
- A lista mantém as **10.000** requisições mais recentes.
- Um app que fala QUIC diretamente, contornando o proxy, não pode ser decodificado. Com a configuração padrão, os apps não são informados sobre o HTTP/3, então ficam no HTTP/2 pelo proxy.
- O proxy é para os **seus próprios** apps e dispositivos. Não o aponte para tráfego que você não tem permissão de inspecionar.

## Linha de comando

```bash
emulabzip net                       # the only running device; prints one line per request
emulabzip net --json | jq 'select(.status >= 400)'
emulabzip net --no-connect --port 9191   # only run the proxy
emulabzip net --rules rules.json    # mock, delay or block requests
emulabzip net --http3               # talk HTTP/3 to servers that offer it
emulabzip net --replay run.ezsession   # answer from a recording; --passthrough lets the rest through
emulabzip net import capture.pcap   # read a packet capture
emulabzip ca install                # make the device trust the certificate
emulabzip ca path                   # where the certificate file is
```

Veja [Linha de comando](cli.md) para todas as opções.
