---
source: 8715ca46d87a
description: O servidor MCP do EmuLabZip deixa um assistente de IA ler logs, crashes e requisições, controlar a tela e gravar sessões nos seus emuladores e simuladores.
---
# Servidor MCP

`emulabzip mcp` deixa um assistente de IA olhar seus emuladores e simuladores e trabalhar com eles: ler logs, crashes e
requisições, controlar a tela, gravar uma sessão, verificar um build. Ele fala o
[Model Context Protocol](https://modelcontextprotocol.io) pela entrada e saída padrão, e usa o mesmo código do app e da linha
de comando, então um dispositivo se comporta do mesmo jeito em todo lugar.

Ele oferece **118 ferramentas** (130 com as mudanças ativadas), **8 prompts** e **13 recursos**. A lista completa, gerada a
partir do próprio servidor para que não fique desatualizada, é a [referência do MCP](mcp-tools.md).

## Conecte um assistente

O app desktop mostra o texto exato para o seu computador em **Settings, AI assistants**, com um botão Copy. Manualmente:

**Claude Code**

```bash
claude mcp add emulabzip -- emulabzip mcp
```

**Claude Desktop**, em `claude_desktop_config.json` (Settings, Developer, Edit Config):

```json
{
  "mcpServers": {
    "emulabzip": { "command": "emulabzip", "args": ["mcp"] }
  }
}
```

**Cursor**, em `~/.cursor/mcp.json` (ou `.cursor/mcp.json` em um projeto): o mesmo bloco `mcpServers` do Claude Desktop.

**VS Code**, em `.vscode/mcp.json`:

```json
{
  "servers": {
    "emulabzip": { "type": "stdio", "command": "emulabzip", "args": ["mcp"] }
  }
}
```

Qualquer outro cliente que inicie um servidor stdio funciona do mesmo jeito. Dê a `command` o caminho completo de `emulabzip`
se ele não estiver no PATH do cliente.

## O que um assistente pode fazer

Toda ferramenta aceita um `device` opcional (um id ou um nome). Quando só um dispositivo está em execução, ele pode ser omitido.

| Peça | Ele usa |
|---------|---------|
| "O que está na tela? Está acessível?" | `screenshot`, `get_layout`, `audit_accessibility`, `check_text`, `simulate_color_blindness` |
| "Abra o app e faça login" | `launch_app`, `tap`, `swipe`, `input_text`, `press_key`, `open_url`, `play_flow`, `flow_create` |
| "Por que ele deu crash?" | `list_crashes`, `get_crash`, `symbolicate_stack`, `read_logs`, `watch_start` |
| "Por que está lento?" | `perf_snapshot`, `record_trace`, `trace_summary`, `measure_startup`, `memory_diff`, `analyze_heap` |
| "O que ele está enviando ao servidor?" | `net_start`, `net_connect_device`, `net_flows`, `net_flow`, `net_export_har`, `openapi_check` |
| "Faça o backend falhar" | `net_rules_set` (mock, delay, block, map), `net_conditions`, `net_replay`, `net_breakpoints` |
| "Acompanhe os logs enquanto eu uso o app" | `watch_start` com `logs`, `perf`, `touches`, `webview_console`, `webview_network`, `react_console` ou `react_actions`, depois `watch_read` |
| "Grave isto e compare com a semana passada" | `session_start`, `session_stop`, `session_summary`, `session_timeline`, `session_compare` |
| "O que há no banco de dados?" | `storage_roots`, `read_database` (SQL somente leitura), `read_preferences`, `list_files`, `read_file` |
| "Este build está pronto?" | `audit_security`, `analyze_package`, `package_budget`, `i18n_sweep`, `visual_compare`, `check_link` |
| "Escreva isto" | `create_bug_report` (deixe de fora o vídeo, as requisições ou os logs com `without`) |

### Watches ao vivo

Os streams (logs, desempenho, toques, WebView e React Native) são *watches*: `watch_start` devolve um id, `watch_read` devolve
o que chegou desde a última leitura (e o que foi perdido se o assistente demorou), e `watch_stop` o encerra. Um watch guarda
os últimos 5.000 itens, termina sozinho após dez minutos sem uma leitura, e no máximo oito rodam ao mesmo tempo.

### Rede, sessão e estado do dispositivo que permanecem

O servidor mantém estado enquanto roda: o proxy, as requisições capturadas, as regras, uma gravação e os watches. Tudo o que
ele mudou em um dispositivo é restaurado quando o assistente se desconecta: a configuração de proxy do dispositivo é limpa e
uma gravação aberta é finalizada. Se uma sessão terminar mal, `net_status` mostra o que ainda está ligado e
`net_disconnect_device` restaura.

As regras de rede são as que o app desktop usa (um arquivo compartilhado), então uma regra definida por um assistente aparece
no app.

## Prompts

Prompts são planos a partir dos quais um assistente pode começar. Escolha um no cliente (no Claude Code, digite `/` e o nome do
prompt).

| Prompt | Para |
|--------|-----|
| `explain_crash` | Achar o crash mais recente, lê-lo com seu mapeamento, dizer o que falhou e como corrigir |
| `why_is_it_slow` | Medir, fazer o trace da parte lenta e apontar a causa |
| `summarize_session` | Contar a história de uma sessão gravada |
| `reproduce_bug` | Seguir uma descrição no dispositivo, gravar e escrever o relatório |
| `debug_request` | Capturar o tráfego e dizer por que uma requisição falhou |
| `release_check` | Verificações de segurança, acessibilidade, tamanho, texto e idioma de uma só vez |
| `check_deep_link` | Verificar um link e ver onde ele cai |
| `mock_backend` | Responder a um host com uma resposta fixa, atraso ou erro |

## Recursos

Um assistente também pode *ler* estes sem chamar uma ferramenta: `emulabzip://devices`, `sessions`, `flows`, `states`,
`links/history`, `rules`, `network/flows` e `guide` (como usar o servidor, por tarefa), além de `emulabzip://sessions/{id}`,
`emulabzip://flows/{id}` (e `/maestro`, `/script`) e `emulabzip://network/flows/{id}`. Um endereço nunca pode nomear um
caminho: os ids são verificados do mesmo jeito que as ferramentas verificam os seus.

## O que um assistente pode e não pode fazer

As ferramentas que olham ficam sempre ativas. As ferramentas que **agem** em um dispositivo (tocar, gravar, conectar o
proxy…) o alteram de verdade: é para isso que o servidor serve. Elas são limitadas pelas mesmas verificações do app (veja
[Controls](controls.md#safety)): um link para `file:` é recusado, os valores são validados e nada chega a um shell sem aspas.

**Destruir ou reescrever dados vem desativado por padrão.** Apagar ou excluir um dispositivo, restaurar um estado, carregar ou
excluir um snapshot de emulador, gravar um arquivo no armazenamento de um app, executar script em uma página web e excluir
sessões, fluxos, mapeamentos e baselines não são oferecidos de forma alguma. Inicie o servidor com `--allow-writes` e eles
aparecem (marcados como destrutivos), e **cada chamada pergunta antes**: o cliente mostra o que vai acontecer ("Wipe
Pixel_Test? All its apps and data are deleted.") e nada roda até você aceitar. Restaurar um estado salva antes os dados como
estão agora.

```bash
claude mcp add emulabzip -- emulabzip mcp --allow-writes
```

- Um cliente que não consegue perguntar a você (sem *elicitation* do MCP) é recusado, com o motivo. Para uma execução sem
  supervisão em um dispositivo descartável, `--allow-writes --no-confirm` pula a pergunta; não use onde houver dados com os
  quais você se importa.
- **Aprovar scripts e plugins, e executar `ci`, nunca são oferecidos**, com ou sem `--allow-writes`: eles executam código,
  então ficam com você. `run_plugin` recusa um plugin que você não ativou, e uma regra de rede que executa um script só é
  aceita para um script que você aprovou no app.
- Arquivos só são gravados nas pastas do próprio EmuLabZip (`exports`, `reports`, `sessions`, `states`, `flows`, `mappings`);
  um assistente nunca informa um caminho para gravar.
- Ele não pode baixar nada. `trace_summary` e `trace_query` precisam do trace processor do Perfetto, que só é baixado depois
  que você concorda no app ou executa `emulabzip trace summary` uma vez.
- Ele lê só o que o dispositivo expõe: os arquivos de apps que você já pode navegar, e só quando são debuggable no Android. As
  ferramentas de Storage são somente leitura (SQL que grava é recusado).
- Ele só fala pela entrada e saída padrão. Não há porta de rede para o próprio protocolo.

Trate um assistente como um colega com acesso ao seu terminal: não o aponte para um dispositivo com dados que você não pode
se dar ao luxo de alterar.

## Experimentando

```bash
# Ask the server what it offers, the way an assistant does
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"try","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | emulabzip mcp
```

Os logs vão para a saída de erro padrão (`EMULABZIP_LOG=debug`); a saída padrão carrega apenas o protocolo.
