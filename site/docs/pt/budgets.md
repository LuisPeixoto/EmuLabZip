---
source: 4044d9ed6c76
description: Defina quanto uma execução pode custar em abertura, memória, CPU, requisições e crashes, e faça o emulabzip ci falhar ao passar do orçamento ou da baseline.
---
# Orçamentos e baselines

Escreva o que uma execução pode custar, e o `emulabzip ci` faz a execução falhar quando ela custar mais.

## `emulabzip.toml`

Coloque-o na pasta do projeto (ou passe `--config`). Todo limite é opcional; uma execução só é limitada pelos que estiverem escritos.

```toml
[budget]
start_time_ms = 800        # the median start time; needs [startup]
memory_peak_mb = 300
cpu_avg_percent = 40
janky_frames_percent = 5
requests = 200
failed_requests = 0
log_errors = 10
crashes = 0

[startup]                  # optional: measure the start time before the command runs
runs = 5
kind = "cold"              # cold, warm or hot

[baseline]
tolerance_percent = 10     # how much worse than the baseline counts as a regression

[size]                     # a build's size: see Build size (package-size.md)
max_total_mb = 25
max_dex_mb = 8
max_native_mb = 12
max_methods = 60000
max_growth_percent = 5

[visual]                   # screenshots: see Visual regression (visual-regression.md)
threshold_percent = 0.1
tolerance = 8
```

`[size]` é verificado em um arquivo de build com `emulabzip size show --budget` e `emulabzip ci --size`; `[visual]` define como
o `emulabzip visual` compara e quais partes da tela ignorar. Veja [Tamanho da build](package-size.md) e
[Regressão visual](visual-regression.md).

Uma chave que não existe é um **erro**, para que um erro de digitação não deixe um limite desligado. Um limite que não pôde
ser medido conta como violado: `start_time_ms` sem uma tabela `[startup]` e sem `--app` falha, em vez de passar em silêncio.

## Orçamentos no `ci`

```bash
emulabzip ci -o run.ezsession --app com.example.app -- ./gradlew connectedCheck
```

Depois da execução, os limites são verificados e listados no relatório em Markdown, em **Budgets**. Se um deles for violado,
o código de saída é 1, a menos que o próprio comando tenha falhado, e então o código dele prevalece.

## Baselines

Compare uma execução com uma sessão anterior, geralmente a última boa:

```bash
emulabzip ci -o run.ezsession --app com.example.app --baseline last-good.ezsession --fail-on-regression -- ./test.sh
emulabzip session compare last-good.ezsession run.ezsession
```

Um número é **pior** quando cresceu mais do que a tolerância (10% por padrão, `--tolerance` em `session compare`). O
relatório lista cada número como melhor, pior ou igual. Sem `--fail-on-regression`, uma regressão é só informada.

No GitHub Actions, guarde a última sessão boa como cache ou artefato da branch padrão e baixe-a antes da execução:

```yaml
- uses: actions/cache@v4
  with:
    path: baseline.ezsession
    key: baseline-${{ github.base_ref || github.ref_name }}
    restore-keys: baseline-
- run: >
    emulabzip ci -o run.ezsession --app com.example.app
    --baseline baseline.ezsession --fail-on-regression -- ./gradlew connectedCheck
- if: github.ref_name == 'main'
  run: cp run.ezsession baseline.ezsession
```

O tempo de abertura é guardado na sessão (`startup.json`), então ele também pode ser comparado. As sessões feitas antes disso
continuam funcionando: o arquivo é opcional e a versão do formato continua sendo 1.
