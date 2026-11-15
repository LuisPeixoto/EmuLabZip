---
source: 75e61f983ac4
description: "Use regras no proxy para mudar o que o app recebe sem tocar no servidor: responda você mesmo, atrase, falhe ou pare uma requisição e edite à mão."
---
# Mocks e breakpoints

As regras deixam você mudar o que um app recebe de volta sem tocar no servidor: responder uma requisição você mesmo, deixá-la lenta, fazê-la
falhar, ou pará-la e editá-la à mão. Elas vivem no proxy, então funcionam em todo dispositivo ao qual a ferramenta [Network](network.md)
está conectada.

## Regras

Uma regra tem um **matcher** (quais requisições) e uma **action** (o que fazer). A primeira regra ativa que casar vence.

| Campo do matcher | Casa com |
|---------------|---------|
| `methods` | Qualquer um destes métodos. Vazio significa qualquer. |
| `host` | Um padrão: `api.example.com`, `*.example.com`. `*` é qualquer sequência de caracteres, `?` qualquer um. |
| `path` | Um padrão para o caminho sem a query: `/v1/users/*`. |
| `urlContains` | Texto que o endereço inteiro (host, caminho e query) precisa conter. |

Todo campo definido precisa casar.

| Action | O que acontece |
|--------|--------------|
| **Answer** (`mock`) | O proxy responde com o seu status, cabeçalhos e corpo, após um atraso opcional. O servidor nunca é consultado. |
| **Delay** | A requisição espera e depois segue. Use para testar uma rede lenta. |
| **Block** | O app recebe um erro de gateway, como se a rede tivesse falhado. Nada chega ao servidor. |
| **Hold** (`breakpoint`) | A requisição, a resposta, ou as duas, param e esperam por você. |

Mais quatro actions enviam uma requisição para um arquivo ou outro servidor, conferem a resposta com um arquivo OpenAPI ou executam um
script WebAssembly, e uma regra pode deixar as de baixo também valerem: veja [Regras de rede](network-rules.md) e
[Scripts de regra](rule-scripts.md).

Os fluxos que uma regra tocou mostram um selo **MOCK** ou **RULE** na lista e nos detalhes.

Uma regra é para uma coisa que você escolheu. Para responder *tudo* a partir de uma gravação, veja [Reproduzindo uma gravação](replay.md). Uma regra
ainda vence a gravação.

## No app

Abra **Network** e depois **Rules**. Escolha um preset (uma resposta JSON, um 500, uma rede lenta, um bloqueio, um breakpoint) ou comece
de uma requisição: selecione-a e pressione **Mock** para obter uma regra que responde como o servidor respondeu, pronta para editar. As regras
valem para a próxima requisição e ficam em `net-rules.json` na pasta de dados do EmuLabZip.

### Breakpoints

Uma requisição ou resposta retida aparece em uma gaveta na parte de baixo da visão Network. Você pode editar os cabeçalhos (um
`name: value` por linha), o corpo (quando é texto) e, em uma resposta, o status. **Continue** envia adiante com as suas
edições, **Abort** a faz falhar, **Continue all** deixa tudo passar. Uma resposta é mostrada decodificada e segue
sem compressão. Se ninguém responder, uma mensagem retida continua sem mudança após 2 minutos, então um app nunca trava de vez.

## Na linha de comando

```bash
emulabzip net --rules rules.json
```

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

Um mock pode levar um corpo binário: defina `"base64": true` e passe o corpo em base64. Os breakpoints precisam de alguém para continuá-los,
então a linha de comando ignora essas regras e avisa.

## Limites

- Um corpo de mock pode ter até 5 MB, e um atraso até 60 segundos.
- As regras veem a requisição decifrada, então o HTTPS precisa do [certificado](network.md#the-certificate) instalado. Os hosts que
  são repassados (apps com certificado fixado) não podem ser alterados.
- Upgrades de WebSocket são casados como qualquer requisição, mas só o handshake pode ser simulado ou bloqueado.
