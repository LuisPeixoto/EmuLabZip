---
source: a4b2edad11c5
description: Audite um app em busca de segredos e dados pessoais no log, no armazenamento e nas requisições, de flags perigosas e de lacunas de privacidade no iOS.
---
# Auditoria de segurança e privacidade

<p align="center">
  <img src="images/audit-security.png" alt="A view Audit com os achados de um app: um bearer token no log, um token de sessão nas preferências, um receiver exportado" />
</p>

`emulabzip audit security <app>` e a view **Audit** procuram o que um app não deveria deixar espalhado: segredos e dados
pessoais no log, nas preferências e em bancos de dados, HTTP puro e segredos em requisições, flags como `debuggable` e
`allowBackup`, componentes que outros apps conseguem alcançar, exceções do App Transport Security e lacunas em um privacy
manifest do iOS. Cada achado diz o que é, onde está e como corrigir.

```bash
emulabzip audit security com.example.shop
emulabzip audit security com.example.shop --har run.har       # also look at recorded requests
emulabzip audit security com.example.shop --only manifest,storage --json
emulabzip audit security com.example.shop --fail-on error     # exit 1 when something at least this serious is found
```

**Segredos nunca são repetidos.** A evidência é reduzida aos dois primeiros e aos dois últimos caracteres e ao tamanho
(`ey…Xk (131 chars)`), no terminal, em JSON, no relatório e por MCP.

## O que ele examina

| Área | Regras |
|------|-------|
| **Manifest** (Android, a partir de `dumpsys package`) | `debuggable`, `allow-backup`, `old-target-sdk` (abaixo da API 34), `exposed-component` (um receiver, service ou activity com um intent filter, exceto o launcher), `content-provider` (listado; o `dumpsys` não diz se ele é exportado) |
| **Manifest** (iOS, `Info.plist`) | `ats-arbitrary-loads`, `ats-arbitrary-loads-scoped` (conteúdo web, mídia), `ats-insecure-domain` (HTTP puro permitido para um domínio), `ats-old-tls` |
| **Privacidade** (iOS) | `no-privacy-manifest`, `tracking-not-declared`, `undeclared-tracking-domain`: os domínios de publicidade e analytics que o app foi visto usando, comparados com o `PrivacyInfo.xcprivacy` |
| **Tráfego** (requisições gravadas) | `cleartext-traffic` (uma vez por host; `localhost`, `10.0.2.2` e `*.local` não são reportados), `secret-in-url` (um token ou chave na query string) e, apenas em HTTP puro, `<kind>-in-cleartext-body` |
| **Log** | `<kind>-in-log` |
| **Armazenamento** | `<kind>-in-storage` (SharedPreferences, DataStore, MMKV, property lists), `secret-in-storage` (uma chave com nome de segredo e com valor), `sensitive-column` (uma coluna SQLite chamada `password`, `token`, `api_key`…) |

`<kind>` é `jwt`, `bearer-token`, `api-key` (formatos de chaves do Google, AWS, Stripe, GitHub e Slack), `secret`, `email` ou
`card-number` (um número que passa na verificação de Luhn, para que números de pedido não sejam confundidos com cartões).
Placeholders como `password=****` e `${TOKEN}` são deixados de lado, assim como datas e dados binários sob um nome que soa
como segredo.

## De onde vêm as requisições

A auditoria lê as requisições do proxy. No app desktop, inicie antes o proxy na view **Network**: a auditoria então inclui o
que ele gravou. Na linha de comando, passe um arquivo HAR (`--har`, de `emulabzip net --har`). Um body só é avaliado quando
a requisição foi por HTTP puro, porque por HTTPS o que o app envia ao servidor é assunto dele.

## O log

O log do dispositivo é compartilhado por todos os apps. O EmuLabZip lê as linhas do processo do próprio app (e as que o
mencionam), então o app precisa estar rodando. Para um app que não está, a área é reportada como **not read** em vez de
varrer as linhas do sistema, que seriam segredos de outras pessoas e números que só parecem cartões.

## No `ci`

```bash
emulabzip ci -o run.ezsession --app com.example.shop --network --fail-on-security warning -- ./gradlew connectedCheck
```

Depois do comando, a auditoria roda (com as requisições que o proxy gravou) e a execução falha quando um achado é pelo menos
tão grave quanto isso. Os achados ficam no resumo em Markdown. Veja o [modo CI](ci.md).

## Limites

- O `dumpsys package` não imprime o atributo `exported`. Um componente com um intent filter é alcançável por outros apps, e
  é isso que é reportado; um sem filtro mas exportado não é visto.
- O armazenamento do Android e os arquivos do app só podem ser lidos em apps **debuggable**, como em [Storage](storage.md).
- Os padrões encontram o que parece um segredo. Um segredo curto ou incomum passa despercebido, e um valor que parece um é
  reportado: leia o achado antes de agir.
- iOS: o log só é lido enquanto o app roda; as declarações de privacidade são comparadas com os domínios vistos nas
  requisições gravadas, então grave algumas antes.
- É uma primeira passada, não um teste de invasão. Ele não olha dentro do código do app nem do servidor dele.

## Para assistentes

Por [MCP](mcp.md), `audit_security` devolve os mesmos achados. Ele lê; não muda nada no dispositivo.
