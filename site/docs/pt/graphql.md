---
source: 57ea062b424d
description: Veja requisições GraphQL na ferramenta Network nomeadas pela operação, com a query e as variáveis lidas, inclusive lotes, e erros na aba Response.
---
# GraphQL

![Uma requisição GraphQL em Network, mostrada como query e variáveis](images/network-graphql.png)

Em **Network**, uma requisição GraphQL recebe o nome do que ela pede, então uma lista de `POST /graphql` fica legível.

- A **Operation** aparece antes do caminho: o `operationName` que o cliente enviou, ou então o nome escrito depois de
  `query`, `mutation` ou `subscription`. Uma operação sem nome aparece como `anonymous query` (ou `mutation`).
  Um lote (uma lista de requisições em um só corpo) mostra os primeiros nomes e quantos mais há.
- As requisições GraphQL são encontradas pelo corpo: um `POST` JSON com uma `query`, um corpo `application/graphql` ou um `GET`
  com `?query=`.
- O filtro também busca nos nomes das operações.
- Aba **Request**: a query como foi escrita, depois as variáveis em JSON e um selo **GraphQL**. Em um lote, cada
  operação tem o seu próprio título.
- Na linha de comando, `emulabzip net` imprime a operação depois de cada requisição.

A resposta de uma chamada GraphQL é JSON como qualquer outra. Um servidor GraphQL geralmente responde `200` mesmo quando a operação
falhou, com a falha em uma lista `errors`: abra a aba **Response** para vê-la.
