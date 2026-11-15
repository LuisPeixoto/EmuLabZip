---
source: a0fef516194b
description: Veja decodificados, em tempo real, os eventos que um app envia ao Segment, Amplitude, Mixpanel e Firebase Analytics, em uma lista própria.
---
# Inspetor de analytics

<p align="center">
  <img src="images/analytics.png" alt="A view Analytics listando os eventos que um app enviou ao Segment e ao Mixpanel, com um aberto" />
</p>

Veja os eventos que um app envia ao **Segment**, **Amplitude**, **Mixpanel** e **Firebase Analytics**, decodificados, no
momento em que acontecem, em uma lista própria.

```bash
emulabzip analytics                          # runs the proxy, points the device at it, prints each event
emulabzip analytics --service segment,mixpanel --json
emulabzip analytics --firebase-log           # Firebase's debug-mode lines from the device log
```

As chamadas de analytics são requisições comuns, então continuam na lista de Network também. O inspetor lê o host, o
endereço, os headers e o body delas, e por isso funciona com tudo o que o proxy grava e com uma sessão ou HAR aberto depois.

## O que é decodificado

| Serviço | Host | O quê |
|---------|------|------|
| Segment | `*.segment.io`, `*.segment.com` | `/v1/track`, `/v1/identify`, `/v1/page`, `/v1/screen`, `/v1/group`, `/v1/alias` e `/v1/batch`: JSON, uma chamada ou um lote |
| Amplitude | `*.amplitude.com` | A API HTTP (`/2/httpapi`, `/batch`: JSON com `events`) e o body de formulário mais antigo (`e=`), com propriedades de evento e de usuário |
| Mixpanel | `*.mixpanel.com` | `/track` e `/import` (`data=` em base64, no body ou na query, ou JSON puro) e as mudanças de perfil de `/engage` |
| Firebase Analytics | `app-measurement.com` | O protobuf em lote de `/a`: nomes de eventos, parâmetros e horários, depois de desfeito o `Content-Encoding` do body |

Cada evento é um serviço, um tipo (`track`, `screen`, `page`, `identify`, `profile`…), um nome, suas propriedades, o usuário
(ou o id anônimo ou do dispositivo) e seu horário quando tem um.

O Firebase também pode ser lido **sem o proxy**. Com o modo debug ligado para o app
(`adb shell setprop debug.firebase.analytics.app <package>`), o Firebase escreve cada evento no log com a tag `FA`, e
`emulabzip analytics --firebase-log` os imprime.

## No desktop

A view **Analytics** lista os eventos das requisições que o proxy gravou, os mais novos primeiro, com um filtro por serviço
e uma busca em nomes e propriedades. Abra um evento para ver as propriedades dele. Inicie o proxy antes, na view **Network**.

## Limites

- SDKs que **fixam o certificado** (pinning) passam sem alteração e não podem ser lidos. O log de debug do Firebase
  funciona para eles.
- O formato em lote do Firebase é o do protocolo de medição do Google como é entendido hoje, e o decodificador foi testado
  com payloads feitos nesse formato, **não com tráfego capturado de um app Firebase de verdade**. Quando o Google o mudar,
  eventos podem faltar: avise em uma issue, com uma captura se puder.
- Os eventos são decodificados sob demanda, não armazenados: um evento em uma requisição que o proxy não guarda mais se
  perdeu.
- Por [MCP](mcp.md) não existe ferramenta de analytics: o servidor não roda um proxy.
