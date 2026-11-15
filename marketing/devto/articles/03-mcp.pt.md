---
title: Dê olhos ao Claude, Cursor ou Copilot no seu emulador com MCP
description: O servidor MCP do EmuLabZip deixa um assistente de IA ler crashes, acompanhar logs e tráfego ao vivo e controlar a tela do emulador, sem mudar dados sem permissão.
tags: ai, mcp, android, devtools
series: EmuLabZip (PT)
cover: docs/images/crashes.png
publish_after: 2026-11-19
pair: 03-mcp
lang: pt
---

Pergunte a um assistente de IA "por que a tela de checkout dá crash?" e ele só consegue chutar, porque não enxerga o seu
dispositivo. O Model Context Protocol (MCP) resolve isso: um servidor expõe ferramentas e o assistente as chama. O
`emulabzip mcp` é um servidor assim para os seus emuladores Android e simuladores iOS.

Ele oferece 118 ferramentas, 8 prompts e 13 recursos, e usa o mesmo código do app desktop e da CLI, então o dispositivo se
comporta igual em qualquer lugar. Neste post você vê como conectar, o que um assistente consegue fazer com ele e quais são as
regras de segurança.

## Conecte

Claude Code:

```bash
claude mcp add emulabzip -- emulabzip mcp
```

Claude Desktop (`claude_desktop_config.json`) e Cursor (`~/.cursor/mcp.json`) usam o mesmo bloco:

```json
{
  "mcpServers": {
    "emulabzip": { "command": "emulabzip", "args": ["mcp"] }
  }
}
```

VS Code (`.vscode/mcp.json`):

```json
{
  "servers": {
    "emulabzip": { "type": "stdio", "command": "emulabzip", "args": ["mcp"] }
  }
}
```

O app desktop mostra esses trechos com o seu caminho já preenchido, em **Settings, AI assistants**, com um botão Copy. O
servidor fala apenas pela entrada e saída padrão: não existe porta de rede.

![Tela Crashes do EmuLabZip, o tipo de dado que o assistente lê](docs/images/crashes.png)

## O que um assistente consegue fazer

Toda ferramenta aceita um `device` opcional, que pode ser omitido quando só há um dispositivo rodando. Alguns exemplos do que
você pode pedir e do que ele usa:

| Peça | Ferramentas |
|---|---|
| "Por que deu crash?" | `list_crashes`, `get_crash`, `symbolicate_stack`, `read_logs` |
| "O que ele está enviando para o servidor?" | `net_start`, `net_connect_device`, `net_flows`, `net_flow`, `net_export_har` |
| "Faça o backend falhar" | `net_rules_set` (mock, delay, block), `net_conditions` |
| "Por que está lento?" | `perf_snapshot`, `record_trace`, `trace_summary`, `measure_startup` |
| "O que tem no banco de dados?" | `storage_roots`, `read_database` (SQL somente leitura), `read_preferences` |
| "Abra o app e faça login" | `launch_app`, `tap`, `swipe`, `input_text`, `press_key` |
| "Grave isso" | `session_start`, `session_stop`, `session_summary`, `session_timeline` |

Os fluxos contínuos são **watches**: `watch_start` devolve um id para logs, desempenho, toques, saída de WebView ou de React
Native, `watch_read` devolve o que chegou desde a última leitura e `watch_stop` encerra. Assim o assistente acompanha os logs
enquanto você usa o app, em vez de ler um único snapshot.

O servidor também guarda estado enquanto está rodando: o proxy, as requisições capturadas, uma gravação. Quando o assistente
se desconecta, a configuração de proxy do dispositivo é restaurada e uma gravação aberta é finalizada.

## Prompts para as tarefas comuns

Prompts são planos de onde um assistente pode partir, e cada um nomeia as ferramentas a chamar: `explain_crash`,
`why_is_it_slow`, `summarize_session`, `reproduce_bug`, `debug_request`, `release_check`, `check_deep_link` e
`mock_backend`. No Claude Code você os escolhe digitando `/`. O prompt `reproduce_bug`, por exemplo, instrui o assistente a
iniciar a gravação de uma sessão, dizer o que vai fazer antes de cada ação, controlar o app com toques e digitação, parar a
gravação e escrever um relatório de bug.

## Seguro por padrão

Um assistente pode olhar tudo, mas não consegue destruir nada, a menos que você permita:

- **Apagar ou excluir um dispositivo, restaurar um estado, escrever um arquivo no armazenamento de um app e executar script
  em uma página web não são oferecidos de forma alguma.** Inicie o servidor com `--allow-writes` e eles aparecem, marcados
  como destrutivos, e cada chamada pede a sua confirmação antes, dizendo exatamente o que vai acontecer ("Wipe Pixel_Test?
  All its apps and data are deleted.").
- Um cliente que não consegue perguntar a você (sem MCP elicitation) é recusado, com o motivo. `--allow-writes --no-confirm`
  pula a pergunta e serve apenas para uso sem supervisão em um dispositivo descartável.
- **Aprovar scripts e plugins e executar comandos de CI nunca são oferecidos**, com ou sem `--allow-writes`.
- As ferramentas de Storage são somente leitura, e arquivos só são gravados nas pastas do próprio EmuLabZip.

```bash
claude mcp add emulabzip -- emulabzip mcp --allow-writes
```

Trate um assistente como um colega com acesso ao seu terminal: não aponte para um dispositivo com dados que você não pode
se dar ao luxo de alterar.

## Experimente

Você precisa da CLI e de um emulador ou simulador rodando:

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

- Guia do MCP: [emulabzip.luispeixoto.dev/pt/docs/mcp/](https://emulabzip.luispeixoto.dev/pt/docs/mcp/)
- Todas as ferramentas, prompts e recursos: [emulabzip.luispeixoto.dev/docs/mcp-tools/](https://emulabzip.luispeixoto.dev/docs/mcp-tools/)
- Site e documentação: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/pt/)
- Código-fonte (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
