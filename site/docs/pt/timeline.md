---
source: ffe4b55ec1e8
description: Veja log, requisições, toques, quadros lentos, GCs e crashes de uma sessão em um só eixo de tempo, com o vídeo da tela gravado em pedaços ao lado.
---
# Linha do tempo e vídeo da tela

<p align="center">
  <img src="images/timeline.png" alt="A aba Timeline: faixas para toques, requisições, log, quadros lentos, GC e crashes, e o vídeo no momento do crash" />
</p>

Uma [sessão](sessions.md) é uma pilha de streams separados: linhas de log, amostras, requisições, crashes. Cada um marca o
tempo do seu jeito, então ver *o que aconteceu logo antes do crash* significava abrir quatro abas e fazer contas. A **linha
do tempo** põe tudo em um só eixo, e o **vídeo da tela** permite que você assista.

## O que está na linha do tempo

| Faixa | O quê | De onde vem |
|------|------|---------------------|
| **Touches** | Taps, toques longos e swipes, com o ponto da tela | `getevent` no touchscreen (Android; veja abaixo) |
| **Requests** | Cada requisição, pelo tempo que levou, em vermelho quando falhou | O proxy (`--network`) |
| **Log** | Avisos e erros | O log |
| **Slow frames** | Quadros que o app demorou demais para desenhar. Um engasgo de trinta quadros é um só evento: uma *rajada* | `dumpsys gfxinfo framestats` (Android) |
| **GC** | Garbage collections, com quanto tempo levaram e pausaram o app | As próprias linhas de log do ART (Android) |
| **Crashes** | Cada crash, no momento em que aconteceu | Os registros de crash do dispositivo |
| **Screenshots**, **Video** | Onde cada screenshot foi tirado e o que cada pedaço de vídeo cobre | A gravação |

## Gravando o que a linha do tempo precisa

```bash
emulabzip session record -o run.ezsession --app com.example.app --network --video --taps
```

- `--taps` grava toques. Eles são lidos do dispositivo de entrada do kernel, então são os toques que chegam lá: um dedo na
  janela do emulador, ou um evento enviado pelo console do emulador (que o app só toma por um clique quando o evento traz
  `ABS_MT_PRESSURE`). `adb shell input tap` é injetado mais acima, dentro do Android, e **não** é visto. O iOS não entrega
  os toques do simulador a nada fora dele, então isso é só para Android. O kernel informa uma posição só quando ela muda,
  então o primeiríssimo toque de uma gravação pode não ter nenhuma: ele é mantido e mostrado como "Tap" sem coordenadas.
- `--video` grava a tela em pedaços de 30 segundos (`--video 10` para 10). Veja [Vídeo da tela](#screen-video).
- Quadros lentos e garbage collections não precisam de opção: são gravados quando há um app, no Android.

No app desktop, a view **Sessions** tem as caixas **Video** e **Touches** ao lado de **Network**.

## Um só relógio

Cada stream marca o tempo do seu jeito: o log imprime a hora local do dispositivo sem ano, as amostras contam desde que
começaram, as requisições usam o relógio deste computador, e os toques e quadros contam desde que o dispositivo ligou.
Quando uma gravação começa, o EmuLabZip pergunta ao dispositivo o relógio, o fuso horário e o uptime dele, e os guarda na
sessão. A linha do tempo os usa para pôr cada stream no relógio deste computador, em milissegundos desde o início da
gravação.

Uma gravação feita antes do EmuLabZip 0.10 não media isso. Os horários do log dela são calculados a partir da primeira
linha (que foi escrita instantes depois do início, o que diz qual devia ser o fuso), e a linha do tempo informa que são
estimados.

## Lendo

No app, abra uma sessão e a aba **Timeline** dela. Passe o mouse sobre um evento para lê-lo, clique nele para ver mais e
para pôr o vídeo ali, arraste para andar pela execução e pressione ⌘ (Ctrl) e role para dar zoom. **Open** salta para a
requisição, o log ou o crash por trás de um evento.

Na linha de comando:

```bash
emulabzip session timeline run.ezsession                    # everything, oldest first
emulabzip session timeline run.ezsession --lane requests --lane crashes
emulabzip session timeline run.ezsession --json             # for scripts
```

```text
+00:04.174  taps      Tap (527, 1464)  (154 ms)
+00:04.206  frames    6 slow frames, worst 118 ms  (216 ms) !
+00:04.749  requests  GET time.akamai.com/ 200  (83 ms)
+00:50.000  crashes   java.lang.IllegalStateException: boom !!
```

Uma faixa guarda no máximo 3.000 eventos. Quando há mais, os erros são mantidos e o resto é distribuído por igual; a
contagem no fim da saída (e, no app, sob o nome da faixa) diz quantos havia.

Um assistente pode lê-la com a ferramenta `session_timeline` do [servidor MCP](mcp.md): por faixa, com limite, do mais
antigo ao mais novo.

## Vídeo da tela

A tela é gravada com `screenrecord` no Android e `simctl io recordVideo` no iOS Simulator, em pedaços curtos. Isso tem três
motivos: o Android para o `screenrecord` depois de três minutos, não importa o que você peça; um arquivo de filme só pode
ser lido depois que a ferramenta o fechou; e um pedaço curto é um clipe que você pode usar como está, sem um editor de
vídeo na máquina.

- O próximo pedaço começa enquanto o último ainda está sendo copiado do dispositivo, então a lacuna entre dois é uma fração
  de segundo no Android e cerca de um terço de segundo no Simulator.
- Os pedaços do Android são redimensionados para que o lado maior tenha 1280 pixels, a 2 megabits por segundo: cerca de 7
  MB para 30 segundos. Os do Simulator têm o tamanho dele, em H.264.
- Parar a gravação mantém o que o pedaço em andamento gravou.
- O vídeo é guardado na sessão sem compressão, já que já está comprimido.

`emulabzip session video run.ezsession -o pieces/` escreve os pedaços como arquivos `.mp4`. No app, a aba Timeline os
reproduz: o vídeo e a linha do tempo se acompanham.

É fácil esquecer do vídeo ao compartilhar um arquivo. Uma tela pode mostrar qualquer coisa: mensagens, um e-mail, uma senha.
Confira o vídeo antes de anexar uma gravação a uma issue pública.

## Em um relatório de bug

Um relatório feito a partir de uma sessão com vídeo inclui o pedaço que contém o crash (o último pedaço, quando não houve
nenhum), e uma **faixa** do último minuto antes do crash: uma linha por tipo de coisa, um ponto para cada uma. A faixa é
desenhada a partir de números e não tem script. Um clipe de até 20 MB faz parte do arquivo HTML; um maior é escrito ao lado
dele como `emulabzip-report.mp4`, ao qual o arquivo se refere, então mantenha os dois juntos. Deixe-o de fora com
`--without video`. Veja [Relatórios de bug](bug-reports.md).
