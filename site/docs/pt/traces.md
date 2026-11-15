---
source: 628549c96a94
description: Grave o que o sistema e um app fizeram em alguns segundos, com Perfetto, simpleperf e Instruments, e descubra por que algo ficou lento, com flame graphs.
---
# Traces

A ferramenta Traces grava o que o sistema e um app fizeram ao longo de alguns segundos e depois ajuda você a descobrir por
que ficou lento. Ela reúne três ferramentas oficiais e guarda a saída delas em uma biblioteca só:

| Gravação | Dispositivo | Ferramenta | O que você recebe no EmuLabZip |
|-----------|--------|------|---------------------------|
| **System trace** | Android 9 e posterior | [Perfetto](https://perfetto.dev) | Um resumo (CPU, quadros, memória, slices mais ocupados), um console SQL e um botão para abrir a linha do tempo completa na UI do Perfetto |
| **CPU profile** | Android 8 e posterior | `simpleperf` | Um flame graph com funções quentes e exportação de pilhas dobradas |
| **Instruments** | iOS Simulator | `xcrun xctrace` | Um flame graph para gravações do Time Profiler e um botão para abrir o `.trace` no Instruments |

<p align="center">
  <img src="images/traces-summary.png" alt="O resumo de um trace de quadros do Perfetto" />
</p>

## Começo rápido

1. Abra **Traces** (<kbd>⌘7</kbd>) e escolha o seu dispositivo. Escolha um app na barra de ferramentas se quiser focar em um.
2. Escolha o que gravar, defina a duração (1 a 300 segundos) e pressione **Record**. <kbd>Stop</kbd> encerra antes
   e mantém o que foi gravado até ali.
3. A gravação abre sozinha. Toda gravação fica na lista **Recordings** até você excluí-la.

Em um terminal:

```bash
emulabzip trace record --preset frames --duration 15 -o frames.pftrace
emulabzip trace summary frames.pftrace
emulabzip trace query frames.pftrace "select name, count(*) n from slice group by name order by n desc limit 10"
emulabzip trace cpu --app com.example.app --duration 10 --folded stacks.txt
```

## System traces (Perfetto)

Quatro presets cobrem as perguntas mais comuns. São configurações de texto comuns do Perfetto, então **Custom** aceita as
suas.

| Preset | Grava | Use para responder |
|--------|---------|------------------|
| **CPU & scheduling** | Trocas e despertares do scheduler, frequência da CPU e estados de ociosidade, a lista de processos | Quais threads usaram a CPU, e ela estava disponível? |
| **Frames & jank** | A linha do tempo de quadros do SurfaceFlinger, mais os slices de view, gráficos, entrada e binder ao redor | Quais quadros atrasaram, e o que estava rodando quando atrasaram? |
| **Memory** | Memória do processo (`mem.rss`) e contadores de memória do sistema a cada 250 ms, mais alocações nativas (`heapprofd`) quando um app é escolhido | Onde a memória cresce? |
| **Custom** | O que a sua configuração disser | Qualquer outra coisa |

A aba **Summary** mostra:

- a duração do trace, quão ocupada a CPU ficou, o número de quadros e o tempo de quadro nos percentis 50, 90 e 99;
- o tempo de CPU por processo e por thread;
- para quadros: os tipos de jank (como o SurfaceFlinger os nomeia) e, por layer, quantos quadros atrasaram. Um quadro conta
  como atrasado quando o app ou o compositor perdeu o prazo; *Prediction Error* e *Buffer Stuffing* sozinhos não contam;
- memória: os menores e maiores contadores do sistema e o pico de memória residente por processo;
- os slices com mais tempo no total.

Uma seção é omitida quando o trace não tem dados para ela. Um trace de memória não tem scheduling, e um trace de CPU não tem
quadros. Use a aba **SQL** para todo o resto: ela executa [PerfettoSQL](https://perfetto.dev/docs/analysis/perfetto-sql-syntax)
(<kbd>⌘</kbd>/<kbd>Ctrl</kbd> + <kbd>Enter</kbd>) com tabelas como `slice`, `sched`, `thread`, `process` e
`counter`, e mostra até 5.000 linhas.

### O trace processor

Ler um trace do Perfetto precisa do `trace_processor_shell` do Google. O EmuLabZip não o inclui. Na primeira vez que você abre
um trace do Perfetto, ele explica o download e pergunta antes:

- ele vem da versão oficial do Perfetto no GitHub (versão fixada no código-fonte, de 12 a 81 MB conforme a
  plataforma);
- o arquivo é conferido com um SHA-256 fixado no código-fonte e é descartado se não bater;
- ele fica em `tools/` dentro da pasta de dados do EmuLabZip e é iniciado somente em `127.0.0.1`, em uma porta livre, quando
  você abre um trace. Ele é parado quando você abre outro trace ou sai do app.

### Abrir na UI do Perfetto

**Open in Perfetto UI** serve o arquivo uma vez a partir de `127.0.0.1` com os cabeçalhos de que o [ui.perfetto.dev](https://ui.perfetto.dev)
precisa e depois abre o seu navegador. A página carrega o trace do seu próprio computador: o arquivo nunca é enviado. O servidor
só responde por esse único arquivo e para depois que ele for buscado ou depois de dois minutos.

## Perfis de CPU (simpleperf)

O `simpleperf` amostra a CPU com pilhas de chamadas. O EmuLabZip o executa no dispositivo, converte o relatório dele no
dispositivo e puxa o resultado, então nenhum NDK é necessário no seu computador.

- **Um app**: o app precisa estar rodando e ser *debuggable* ou *profileable*. Caso contrário, o simpleperf recusa e o EmuLabZip
  informa. Ele não espera por um app que não está rodando.
- **Sistema inteiro**: amostra todos os processos. Precisa de `adb root`, então use uma imagem de emulador sem Google Play.

O flame graph é um icicle: a chamada mais externa em cima, largura proporcional ao tempo de CPU. Clique em um quadro para dar
zoom, use o breadcrumb para afastar, digite para destacar uma função e escolha outro processo ou thread acima dele. As cores
dizem de quem é o código de um quadro:

| Cor | Significado |
|--------|-----------|
| Azul | O código do próprio app: apps instalados, as bibliotecas deles e métodos do app compilados por JIT |
| Verde | Código gerenciado da plataforma: o framework do Android, o ART, `*.jar`, `*.odex`, `*.oat` |
| Laranja | Bibliotecas nativas (`.so`) |
| Cinza | O kernel |

<p align="center">
  <img src="images/traces-flame.png" alt="Um perfil de CPU do sistema inteiro como flame graph" />
</p>

Sob o gráfico, **Hot functions** lista onde o tempo foi gasto na própria função e não no que ela chamou.
**Folded stacks** salva a árvore de chamadas para o `flamegraph.pl` e o speedscope. O `perf.data` bruto fica ao lado da
gravação; *Show in folder* o revela.

## Instruments (iOS Simulator)

O EmuLabZip executa `xcrun xctrace record` para qualquer template que `xctrace list templates` informe, anexado ao app que você
escolheu (ou a todos os processos). O resultado é um documento `.trace`: **Open in Instruments** o abre. Para **Time Profiler**
e **CPU Profiler**, o EmuLabZip também exporta as amostras (`xctrace export`) e desenha o mesmo flame graph.

Requisitos:

- Xcode, com `xcrun xctrace` disponível.
- **Developer Mode** no Mac. Sem ele, o `xctrace` espera por uma aprovação que ninguém pode dar. O EmuLabZip verifica
  `DevToolsSecurity -status` antes e diz o que fazer: execute `sudo DevToolsSecurity -enable` em um terminal.
- Para um app, ele precisa estar rodando no simulador.

O leitor da exportação do Time Profiler segue o formato que o `xctrace` documenta (linhas com uma thread, um peso e uma pilha,
com elementos repetidos escritos uma vez e referenciados por id). Ele é coberto por testes em uma exportação escrita à mão;
gravações de um Mac com Developer Mode não estavam disponíveis enquanto ele foi escrito. Se uma gravação abrir sem flame graph, o
`.trace` continua intacto e abre no Instruments. Abra uma issue com a versão do Xcode.

## Onde as gravações ficam

Cada gravação é uma pasta em `traces/` dentro da pasta de dados do EmuLabZip (`EMULABZIP_HOME` a move):

```text
traces/20261023-184501-3f2a/
    meta.json          what was recorded, where and when
    trace.pftrace      or profile.pb (+ perf.data), or recording.trace (+ time-profile.xml)
```

`emulabzip trace list` as imprime. Excluir uma no app remove a pasta dela. Nada é escrito em um dispositivo sem ser removido
de novo: configurações, gravações brutas e relatórios são apagados do dispositivo quando uma gravação termina,
quer tenha funcionado, falhado ou sido interrompida.

## Limites

- No máximo 300 segundos por gravação e 1 GB por trace do Perfetto.
- O Perfetto precisa do Android 9 (API 28) ou posterior. No Android 9 e 10, o EmuLabZip liga `persist.traced.enable`, já que
  a imagem do sistema o traz desligado.
