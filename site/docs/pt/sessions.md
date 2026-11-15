---
source: d9e8d27bf968
description: Grave tudo o que acontece em um dispositivo enquanto um app é usado, em um único arquivo .ezsession que você abre depois, anexa a um bug ou coleta do CI.
---
# Sessões

Uma **sessão** é uma gravação do que aconteceu em um dispositivo enquanto um app era usado, guardada em um único arquivo que
você pode abrir depois, anexar a um relatório de bug ou coletar do CI.

| Dentro | O quê |
|--------|------|
| `session.json` | O dispositivo, o app, quando começou e terminou |
| `logs.jsonl` | Cada linha de log (até 500.000) |
| `perf.jsonl` | Amostras de CPU, memória, taxa de quadros e bateria, uma por segundo |
| `network.har` | As requisições, quando a rede foi gravada, com mensagens WebSocket |
| `crashes/` | Todo crash, crash nativo e ANR que aconteceu *durante* a execução, com o seu relatório |
| `screens/` | Screenshots tirados a cada poucos segundos, quando pedido, com o `index.jsonl` dizendo quando |
| `input.jsonl` | Toques (taps, toques longos, swipes), quando pedido, no Android |
| `frames.jsonl` | Quadros que o app desenhou devagar demais, no Android |
| `video/` | A gravação da tela em pedaços, quando pedida, com o `index.jsonl` dizendo o que cada um cobre |

O `session.json` também guarda o **relógio** do dispositivo, para que os streams possam ser postos em um só eixo: veja
[Linha do tempo e vídeo da tela](timeline.md).

Um arquivo `.ezsession` é um zip de JSON puro, um arquivo HAR, PNGs e MP4s, então outras ferramentas também podem lê-lo. O
formato está na versão 2; um arquivo da versão 1 do EmuLabZip 0.9 ou anterior ainda abre.

## Gravando

No app, abra **Sessions**, escolha o app na barra de ferramentas e pressione **Record**. Se quiser, marque **Network** (o
dispositivo precisa estar conectado antes na view [Network](network.md)), **Video** e **Touches**, e escolha com que
frequência tirar um screenshot. Pressione
**Stop and save**; a sessão abre na hora. As sessões gravadas aqui ficam listadas à esquerda.

Na linha de comando:

```bash
emulabzip session record -o run.ezsession --app com.example.app --screens 5 --network
# ... use the app, then Ctrl+C. Or add --duration 60 to stop by itself.
emulabzip session record -o run.ezsession --app com.example.app --video --taps   # the screen, and touches (Android)
```

Com `--network`, o dispositivo aponta para o proxy durante a execução e é restaurado depois, mesmo que algo falhe.

Se uma gravação for interrompida à força (um crash, um notebook fechado), o que foi escrito continua lá, em uma pasta
chamada `run.ezsession.partial`. Abra-a como um arquivo; o relatório dela diz "cut short".

## Lendo

```bash
emulabzip session show run.ezsession            # for people
emulabzip session show run.ezsession --json     # for scripts
emulabzip session show run.ezsession --markdown # for a pull request comment
```

O relatório tem a duração, os erros do log e os mais repetidos (números são ignorados, então "retry 3 of 5" e "retry 4 of
5" contam como um só), crashes, requisições com as mais lentas, e picos de CPU e memória.

No app, a sessão aberta tem abas: **Overview**, **Logs** (filtre por nível e texto), **Requests** (headers e body de cada
uma), **Timeline** (toques, requisições, problemas do log, quadros lentos, garbage collections e crashes em um só eixo, com
o vídeo que o acompanha, e os gráficos de CPU e memória abaixo) e **Screens**. **Replay network** responde às requisições
de um app a partir da gravação: veja [Reproduzindo uma gravação](replay.md).

Uma sessão também é o jeito mais fácil de relatar um bug depois: **Bug report** na sessão aberta, ou `emulabzip report
--session run.ezsession`, empacota tudo em um único arquivo HTML. Veja [Relatórios de bug](bug-reports.md).

```bash
emulabzip session timeline run.ezsession        # everything on one axis
emulabzip session video run.ezsession -o clips/ # the pieces of the screen recording
```

Um assistente pode ler sessões com as ferramentas `list_sessions`, `session_summary` e `session_timeline` do
[servidor MCP](mcp.md).

## Bom saber

- Sessões podem conter o que o seu app registra no log e envia: tokens, endereços, dados pessoais. Uma gravação de tela pode
  mostrar qualquer coisa que estava na tela. Trate-as como logs e confira antes de anexar uma a uma issue pública.
- Uma sessão gravada por um EmuLabZip mais novo (um formato mais novo) é recusada com uma mensagem pedindo que você
  atualize.
- Arquivos de sessão só são escritos em lugares que você escolhe ou na pasta `sessions` do próprio EmuLabZip.

Veja o [modo CI](ci.md) para gravar em volta de uma execução de testes.
