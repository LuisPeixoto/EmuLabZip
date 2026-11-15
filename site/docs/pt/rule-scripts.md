---
source: 45af944a7748
description: "Escreva scripts WebAssembly que inspecionam uma requisição ou resposta e decidem o que fazer: adicionar cabeçalho, ocultar campo ou falhar, em sandbox."
---
# Scripts de regra

Um **script** é um pequeno programa WebAssembly que olha uma requisição ou uma resposta e decide o que acontece com ela: adicionar um
cabeçalho, ocultar um campo, responder por conta própria, fazer uma em cada três requisições falhar. Uma regra com a action **Script** o executa para toda
requisição que a regra casar.

Um script roda em sandbox. Ele não pode ler arquivos, acessar a rede, ler o relógio nem tocar em mais nada do seu computador:
a única função que ele pode importar é `env.log`. Ele é interrompido após uma quantidade fixa de trabalho e de memória. E ele
só roda depois que **você aprovou os bytes exatos dele**.

## Aprovando

Um arquivo de script não faz nada até ser aprovado, no diálogo da regra (**Approve this script**, que mostra o SHA-256 dele e
o que ele examina) ou na linha de comando:

```bash
emulabzip script show add-header.wasm       # what it does, and whether it is approved
emulabzip script approve add-header.wasm
emulabzip script list
emulabzip script revoke 09e74ff6            # a file, or the start of its hash
```

A aprovação vale para o SHA-256 do arquivo: mude um byte e ele precisa ser aprovado de novo. Aprove apenas scripts em que você confia: um
script vê as requisições e respostas que a regra dele casa, com seus cabeçalhos e corpos. Um fluxo cujo script não rodou diz
o motivo nos detalhes.

![O diálogo Rules com uma regra de script que não foi aprovada](images/rules-script.png)

## A interface

Um script é um módulo que exporta:

| Export | Tipo | |
|--------|------|-|
| `memory` | memory | A memória linear do script. |
| `alloc` | `(len: i32) -> i32` | Um ponteiro para `len` bytes nos quais o host pode escrever. |
| `on_request` | `(ptr: i32, len: i32) -> i64` | Chamada com uma requisição. Opcional. |
| `on_response` | `(ptr: i32, len: i32) -> i64` | Chamada com uma resposta. Opcional. |

Pelo menos um entre `on_request` e `on_response` é necessário. O host escreve um documento JSON em `ptr` (`len` bytes) e a
função retorna `(out_ptr << 32) | out_len`, onde está a resposta dela, também em JSON.

**Entrada**

```json
{ "method": "GET", "url": "https://api.example.com/users?id=1",
  "headers": [["accept", "application/json"]],
  "body": "<base64>", "bodyOmitted": false,
  "status": 200 }
```

`status` só existe em uma resposta. O corpo de uma resposta é decodificado (gzip, brotli, zstd). Um corpo de mais de 1 MB não é
entregue: `bodyOmitted` é `true` e `body` fica vazio.

**Resposta**

```json
{ "action": "continue", "status": 201, "headers": [["x-by", "me"]], "body": "<base64>", "text": "…", "message": "…" }
```

- `"continue"` (ou nenhuma `action`) deixa a requisição seguir ou a resposta passar. `headers`, `body` (base64) ou `text` e, em uma
  resposta, `status`, substituem o que havia; o que fica de fora não muda. Uma lista de cabeçalhos **substitui** a lista inteira, então
  comece pela da entrada.
- `"replace"` (em uma requisição) responde na hora com esse `status` (200 por padrão), `headers` e corpo, e o servidor nunca
  é consultado. Em uma resposta, é o mesmo que `continue`.
- `"block"` faz a requisição falhar com um erro de gateway, dizendo `message`.

Um script que dá trap, fica sem trabalho, responde algo que não é válido ou pede algo que não lhe é dado **não**
quebra a requisição: ela passa como se o script não existisse, e o motivo fica nos detalhes do fluxo.

## Limites

- **Trabalho:** 10 milhões de unidades de fuel por chamada, aproximadamente o mesmo número de instruções.
- **Memória:** 64 MB. Um módulo pode ter 2 MB e uma resposta 8 MB. No máximo 20 linhas de `env.log`, de 1 KB cada.
- Uma nova instância a cada chamada, então nada que um script guarde sobrevive até a próxima requisição; use a URL quando uma escolha precisar ser
  repetível.
- Nenhum import além de `env.log(ptr: i32, len: i32)`, que escreve uma linha que aparece no fluxo.

## Escrevendo um

Qualquer linguagem que compile para WebAssembly funciona. `tools/rule-scripts` tem três exemplos em Rust, cada um compilado para
`wasm32-unknown-unknown`, com um pequeno crate auxiliar que faz o empacotamento:

| Script | O que faz |
|--------|--------------|
| `add-header.wasm` | Adiciona `x-emulabzip-script` a toda requisição e resposta. |
| `redact-json.wasm` | Substitui o valor de `email`, `token`, `password` e `authorization` em respostas JSON por `***`. |
| `flaky.wasm` | Responde 503 para cerca de um terço dos endereços, escolhidos a partir da URL para que o mesmo sempre falhe. |

```bash
cd tools/rule-scripts && ./build.sh          # needs: rustup target add wasm32-unknown-unknown
```

Os arquivos em `dist/` já estão compilados. Adicione o script a uma regra, aprove-o e observe o fluxo.
