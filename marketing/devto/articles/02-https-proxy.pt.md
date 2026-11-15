---
title: Veja, simule e deixe lento o tráfego HTTPS do seu app no emulador Android
description: Capture HTTPS decifrado de um emulador, responda requisições com mocks, segure-as em breakpoints e teste Slow 3G com um proxy open source e gratuito do EmuLabZip.
tags: android, networking, testing, opensource
series: EmuLabZip (PT)
cover: docs/images/network.png
publish_after: 2026-11-17
pair: 02-https-proxy
lang: pt
---

"Na minha máquina funciona" geralmente quer dizer "o backend respondeu rápido com um 200 bonitinho". Para ver como o seu app
se comporta quando a API está lenta, devolve um 500 ou uma lista vazia, você normalmente mexe no servidor, ou configura um
proxy, instala um certificado e clica pela interface de regras dele.

O EmuLabZip tem um proxy de depuração embutido. Este post mostra como usá-lo em um emulador Android, da captura de HTTPS
até simular respostas e testar uma rede lenta, sem encostar no seu backend.

![Requisições ao vivo de um emulador Android na view Network do EmuLabZip](docs/images/network.png)

## 1. Capture HTTPS

Abra um emulador e faça ele confiar uma vez na autoridade certificadora do EmuLabZip:

```bash
emulabzip ca install
emulabzip net --har capture.har
```

`emulabzip net` roda o proxy no seu computador, aponta o dispositivo para ele, imprime uma linha por requisição e, quando
você aperta `Ctrl+C`, restaura a configuração de proxy do dispositivo. `--har` também grava o tráfego em um arquivo HAR que
você pode abrir em outras ferramentas. Com `--json`, cada requisição vira uma linha de JSON, então dá para filtrar:

```bash
emulabzip net --json | jq 'select(.status >= 400)'
```

O app desktop faz o mesmo na view **Network**: cada requisição com seus headers, corpos decodificados e tempos, e qual versão
do HTTP ela usou.

### Quais apps confiam no certificado

Até onde a confiança chega depende do dispositivo:

| Dispositivo | Resultado |
|---|---|
| iOS Simulator | Todos os apps confiam (reinicie os apps que já estavam abertos) |
| Emulador Android cuja imagem permite `adb root` (Google APIs, AOSP) | Adicionado ao store do sistema: todos os apps confiam até o dispositivo reiniciar |
| Imagem Android do Google Play, ou dispositivo físico | Instalado como certificado de usuário: Chrome e apps que aderirem |

Desde o Android 7, os apps ignoram certificados de usuário, a menos que peçam por eles. No seu build de **debug**, adicione
uma network security config que confie neles. Builds de release não são afetados, porque `debug-overrides` só vale para apps
debugáveis:

```xml
<network-security-config>
    <debug-overrides>
        <trust-anchors>
            <certificates src="system" />
            <certificates src="user" />
        </trust-anchors>
    </debug-overrides>
</network-security-config>
```

Apps que fazem pinning do certificado passam sem alteração e aparecem como **Connections**, porque o EmuLabZip não consegue
lê-los.

## 2. Responda requisições você mesmo

As regras ficam no proxy. Uma regra tem um matcher (quais requisições) e uma ação. A primeira regra habilitada que combinar
vence. Salve isto como `rules.json`:

```json
[
  {
    "name": "stub the profile",
    "matcher": { "host": "api.example.com", "path": "/v1/profile" },
    "action": { "type": "mock", "status": 200,
                "headers": [["content-type", "application/json"]],
                "body": "{\"name\": \"Ada\"}" }
  },
  { "name": "slow uploads", "matcher": { "methods": ["POST"] }, "action": { "type": "delay", "ms": 3000 } },
  { "name": "no analytics", "matcher": { "host": "*.analytics.example.com" }, "action": { "type": "block" } }
]
```

```bash
emulabzip net --rules rules.json
```

Agora a requisição do perfil nunca chega ao servidor, todo `POST` espera três segundos e as chamadas de analytics falham como
um erro de rede. Os fluxos que uma regra tocou ganham um selo **MOCK** ou **RULE** na lista. No app, você também pode
selecionar uma requisição capturada e apertar **Mock** para obter uma regra que responde do mesmo jeito que o servidor
respondeu, pronta para editar.

Há mais ações: responder com um arquivo local, encaminhar para outro servidor (para apontar o app para o seu notebook),
conferir respostas contra um arquivo OpenAPI e scripts WebAssembly em sandbox. Tudo isso está no
[guia de regras de rede](https://emulabzip.luispeixoto.dev/pt/docs/network-rules/).

## 3. Segure uma requisição em um breakpoint

Uma regra também pode **segurar** uma requisição, sua resposta, ou as duas. O app mostra a mensagem em uma gaveta na parte
de baixo de **Network**, onde você pode editar os headers, o corpo (quando for texto) e o status de uma resposta.
**Continue** envia com as suas edições, **Abort** faz ela falhar. Se ninguém responder, uma mensagem retida segue adiante sem
alterações depois de dois minutos, então seu app nunca fica travado para sempre.

## 4. Teste uma rede ruim

O proxy fica no meio de tudo que o dispositivo envia, então ele consegue moldar o link. Os presets são Fast 3G, Slow 3G, Slow
Wi-Fi, High latency, Packet loss e Offline:

```bash
emulabzip net --conditions slow-3g
emulabzip net --down 400 --latency 100
emulabzip net --conditions offline
```

A mudança vale na hora, inclusive para conexões que já estão abertas. Uma ressalva: o HTTP/3 usa QUIC sobre UDP, que não
passa por esse link, então desligue o HTTP/3 para ver um app deixado lento.

## Limites que vale conhecer

- O corpo de um mock pode ter até 5 MB, e um delay até 60 segundos.
- Hosts que passam direto (apps com pinning) não podem ser alterados.
- Em upgrades de WebSocket, só o handshake pode ser simulado ou bloqueado.

## Experimente

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

- Guia de Network: [emulabzip.luispeixoto.dev/pt/docs/network/](https://emulabzip.luispeixoto.dev/pt/docs/network/)
- Mocks e breakpoints: [emulabzip.luispeixoto.dev/pt/docs/mocks/](https://emulabzip.luispeixoto.dev/pt/docs/mocks/)
- Site e toda a documentação: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/pt/)
- Código-fonte (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
