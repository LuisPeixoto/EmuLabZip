---
source: 77184675abce
description: Faça o app rodar sem internet respondendo as requisições a partir de uma gravação, para reproduzir bugs de backend e manter testes de CI repetíveis.
---
# Reproduzindo uma gravação

<p align="center">
  <img src="images/replay.png" alt="Network com uma gravação reproduzida: três requisições respondidas, uma que não estava na gravação" />
</p>

Um **replay** responde às requisições do app a partir de uma gravação, então os servidores nunca são consultados. O app roda
sem internet, contra o que um servidor disse uma vez. Serve para bugs cujo backend mudou desde então e para execuções em CI
que precisam dar o mesmo resultado toda vez.

```bash
# Record once, with the network.
emulabzip session record -o run.ezsession --app com.example.app --network

# Run the app against it. Press Ctrl+C to stop.
emulabzip net --replay run.ezsession
```

Uma [sessão](sessions.md) com a sua rede, ou qualquer arquivo HAR (o export de um navegador ou `emulabzip net --har`), é uma
gravação.

## O que ele faz

O proxy consulta a gravação antes de perguntar a um servidor. Uma requisição que está nela recebe a resposta gravada: o
status, os headers e o body, como eram. Uma requisição que **não** está nela falha com um `504` que diz *not in recording*
e aparece na lista de requisições marcada como falha. Ela nunca é enviada ao servidor. Esse é o ponto: um teste que depende
de um endereço de que ninguém te falou falha, em vez de usar a rede em silêncio.

- As requisições são casadas pelo método, o host, o path e a query. A ordem dos parâmetros da query não importa; a porta
  também não.
- Uma requisição gravada **mais de uma vez** recebe as respostas na ordem em que foram gravadas e, depois, a última de novo.
  Um contador que disse 1 e depois 2 diz 1, depois 2, depois 2.
- Quando requisições ao mesmo endereço diferem no body (uma query GraphQL, por exemplo), a de mesmo body é preferida.
- HTTPS funciona, porque o proxy encerra o TLS ele mesmo (o dispositivo precisa confiar no
  [certificado](network.md#the-certificate) como em qualquer captura).
- Uma regra de [Mocks](mocks.md) ainda vence a gravação, porque você a escreveu de propósito.

## O que ele não faz

Enquanto uma gravação é reproduzida, o que não pode ser respondido a partir de um arquivo HAR é recusado em vez de deixado
passar para a internet:

- **WebSockets**. As mensagens deles estão no arquivo, mas não são reproduzidas.
- **Chamadas gRPC**. Um arquivo HAR não tem trailers, então a chamada não poderia terminar.
- **Conexões que o proxy não consegue decodificar**: outros protocolos e apps que não aceitam o certificado do EmuLabZip.
- **Enviar uma requisição de novo** pela view Network.

Requisições que falharam quando foram gravadas não estão na gravação.

## Deixando algumas passarem

```bash
emulabzip net --replay run.ezsession --passthrough
```

Com `--passthrough`, uma requisição que não está na gravação vai para o servidor real. Use quando você só quer congelar
parte do que o app faz. `--timing` faz cada resposta esperar tanto quanto o servidor esperou, para um app que se comporta
de modo diferente quando a rede está lenta.

## No CI

```bash
emulabzip ci -o run.ezsession --network --replay baseline.ezsession --fail-on-miss -- ./gradlew connectedCheck
```

A execução se grava como de costume, com as requisições respondidas a partir de `baseline.ezsession`. `--fail-on-miss` a
faz falhar quando o app pediu algo que não está na gravação, e o fim da execução lista o quê. `session record` também aceita
`--replay`. Os dois precisam de `--network`.

## No app

Em **Network**, **Replay → Choose a file…** carrega um arquivo HAR ou uma sessão. Uma barra mostra o que está sendo
reproduzido, quantas requisições foram respondidas e quantas não foram (e quais, em **Not recorded**). Em **Sessions**,
**Replay network** carrega a sessão aberta. Inicie o proxy e conecte o dispositivo como de costume; **Stop replay** volta
para os servidores.

## Bom saber

- A gravação é lida antes de qualquer outra coisa, então um arquivo errado ou uma sessão sem rede é explicado de imediato.
- As respostas são os bodies gravados, decodificados. `Content-Encoding`, `Content-Length` e `Alt-Svc` não são repetidos,
  para que o app não seja instruído a usar uma compressão que o body não tem mais, nem a tentar HTTP/3 contornando o proxy.
- Uma gravação guarda o que o servidor enviou, incluindo tokens e dados pessoais. Confira antes de compartilhar.
