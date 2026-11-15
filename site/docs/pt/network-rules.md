---
source: 88df05457f3f
description: Regras de rede que respondem com um arquivo local, enviam a requisição a outro servidor, validam a resposta contra um OpenAPI e se encadeiam.
---
# Regras de rede: arquivos, outros servidores e contratos

[Mocks e breakpoints](mocks.md) respondem, atrasam, bloqueiam ou seguram uma requisição. Mais quatro actions atuam na forma como uma requisição é
respondida, e as regras podem ser encadeadas.

| Action | O que acontece |
|--------|--------------|
| **File** (`mapLocal`) | Responde com um arquivo neste computador, ou com o arquivo de uma pasta que o caminho nomeia. O servidor não é consultado. |
| **Server** (`mapRemote`) | Envia a requisição a outro servidor, como se o app a tivesse pedido a ele. |
| **Contract** (`validate`) | Confere a resposta com um arquivo OpenAPI e mostra o que não bate. Não muda nada. |
| **Script** (`script`) | Executa um [script](rule-scripts.md) WebAssembly na requisição e na resposta. |

No app, elas ficam em **Network → Rules**, com seletores de arquivo. Na linha de comando, vão no mesmo arquivo de `--rules`.

## Map local

```json
{ "name": "bundled assets", "matcher": { "host": "cdn.example.com", "path": "/assets/*" },
  "action": { "type": "mapLocal", "path": "/Users/me/app/dist" } }
```

Com uma pasta, a parte do caminho antes do padrão da própria regra é descartada: com `/assets/*`, uma requisição para
`/assets/app.js` recebe `app.js` da pasta, e uma requisição para uma pasta recebe o `index.html` dela. Com um arquivo, o arquivo é a
resposta de toda requisição que a regra casar. O content type vem da extensão (ou de `contentType`), e `status`
(200 por padrão) é o status da resposta. **Nada fora da pasta é servido jamais**: `..`, um `%2e%2e` codificado, uma
unidade ou um link simbólico que leve para fora são recusados. Um arquivo pode ter até 50 MB.

O fluxo mostra **MOCK** e o arquivo que foi servido.

## Map remote

```json
{ "name": "use my laptop", "matcher": { "host": "api.example.com", "path": "/api/*" },
  "action": { "type": "mapRemote", "scheme": "http", "host": "localhost", "port": 3000, "pathPrefix": "/v2/" } }
```

O que a regra deixa de fora continua como o app pediu, então só um host e uma porta enviam o mesmo caminho a um servidor local. `pathPrefix`
substitui a parte do caminho antes do primeiro `*` do padrão: `/api/users` vira `/v2/users`. Um novo esquema ou host usa a
porta usual do esquema, a menos que uma seja informada. O novo servidor recebe o próprio host, a menos que `"preserveHost": true`. A visão Network
continua mostrando o que o app pediu e diz para onde a requisição foi enviada. O map remote não se aplica a chamadas gRPC.

## Contract

```json
{ "name": "the API contract", "matcher": { "host": "api.example.com" },
  "action": { "type": "validate", "spec": "/Users/me/app/openapi.yaml" }, "thenContinue": true }
```

Cada resposta é conferida, depois de enviada, com uma descrição **OpenAPI 3.0 ou 3.1** em JSON ou YAML (o arquivo é
lido quando a regra é salva, então um arquivo que não pode ser usado é recusado na hora). A operação vem do método e do
caminho (`/users/{id}`, com os caminhos base dos servidores), o schema vem do status (`200`, depois `2XX`, depois `default`) e do
content type, e o corpo é validado. O fluxo ganha uma aba **Contract** e um selo na lista:

- a resposta **bate** (`ok`);
- ela **não bate**: cada problema vem com um JSON pointer para o corpo (`/items/0/name`) e uma mensagem, e um status
  ou content type que o spec não descreve também é um problema;
- o spec **não descreve** a requisição (`unknown`), ou não tem **schema** para conferir esta resposta, ou o corpo não é
  JSON (`unchecked`).

![Uma requisição com duas violações do seu arquivo OpenAPI, na aba Contract](images/network-contract.png)

Offline, `emulabzip openapi check --spec api.yaml --har run.har` confere as respostas de um arquivo HAR e sai com erro
quando uma não bate, que é o que uma etapa de CI quer. Só as respostas são conferidas (não as requisições), só corpos JSON, e
só `$ref`s dentro do arquivo. Um arquivo Swagger 2.0 é recusado com uma mensagem para convertê-lo.

## Encadeando regras

A primeira regra que casar decide, a menos que diga `"thenContinue": true`. Uma regra que **muda** a requisição (um atraso, um map
para outro servidor, uma conferência de contrato, um script) pode passar a requisição às regras de baixo; uma regra que **responde**, bloqueia ou
segura a requisição (uma resposta, um arquivo, um bloqueio, um breakpoint) sempre encerra a busca. Então um atraso seguido de um arquivo responde
devagar a partir do arquivo, e uma conferência de contrato seguida de um mock confere o mock. O fluxo lista toda regra que foi aplicada.
