---
title: Pegue regressões de desempenho no CI com uma sessão gravada
description: Envolva seus testes instrumentados com emulabzip ci para gravar logs, requisições e crashes, impor orçamentos e comparar com a última execução boa no GitHub Actions.
tags: android, ci, testing, performance
series: EmuLabZip (PT)
cover: docs/images/sessions.png
publish_after: 2026-12-01
pair: 06-ci-sessions
lang: pt
---

Um teste de UI que passa só diz que o app não deu crash naquela execução. Ele não avisa que o tempo de abertura cresceu um
segundo, que o pico de memória subiu 40% ou que três requisições começaram a falhar e o app engoliu os erros. Quando alguém
percebe, a mudança que causou o problema já está enterrada embaixo de uma dúzia de commits.

O EmuLabZip consegue gravar o que aconteceu no dispositivo durante qualquer comando e cobrar a execução contra limites que
você define. Neste post, isso vai para um job do GitHub Actions.

![Uma sessão gravada: um crash, erros repetidos e requisições lentas](docs/images/sessions.png)

## Grave uma execução

O `emulabzip ci` grava uma **sessão** em volta de qualquer comando, em geral uma execução de testes, e depois sai com o
próprio código de saída do comando. Seu pipeline se comporta como antes, e você ganha um arquivo para consultar quando algo
der errado:

```bash
emulabzip ci -o run.ezsession --app com.example.app -- ./gradlew connectedCheck
```

Ele começa a gravar, roda o comando (a saída dele passa sem alteração), para, escreve a sessão e imprime um resultado em uma
linha. Uma sessão guarda o log, amostras de CPU e memória, requisições (com `--network`), crashes, capturas de tela a cada N
segundos (`--screens`) e, opcionalmente, o vídeo da tela e os toques. O `--fail-on-crash` faz a execução falhar quando o app
deu crash durante ela, mesmo que o comando tenha passado.

Leia o arquivo depois:

```bash
emulabzip session show run.ezsession
emulabzip session show run.ezsession --markdown
emulabzip session timeline run.ezsession
```

O `--markdown` foi feito para um comentário de pull request, e o `emulabzip ci --summary report.md` já escreve esse arquivo
para você. No GitHub Actions, se a variável `GITHUB_STEP_SUMMARY` estiver definida, o relatório é acrescentado a ela e
aparece na página da execução, sem passos extras.

## Escreva os limites

Coloque um `emulabzip.toml` no projeto:

```toml
[budget]
start_time_ms = 800        # the median start time; needs [startup]
memory_peak_mb = 300
cpu_avg_percent = 40
janky_frames_percent = 5
failed_requests = 0
log_errors = 10
crashes = 0

[startup]                  # measure the start time before the command runs
runs = 5
kind = "cold"

[baseline]
tolerance_percent = 10
```

Depois da execução, os limites são verificados e listados em **Budgets** no relatório. Se algum for estourado, o código de
saída é 1, a não ser que o próprio comando tenha falhado: nesse caso, o código dele vence. Uma chave que não existe é erro,
então um erro de digitação não deixa um limite desligado, e um limite que não pôde ser medido conta como estourado em vez de
passar em silêncio.

## Compare com a última execução boa

Um orçamento pega um problema absoluto. Uma baseline pega o "pior do que antes":

```bash
emulabzip ci -o run.ezsession --app com.example.app \
  --baseline last-good.ezsession --fail-on-regression -- ./test.sh
emulabzip session compare last-good.ezsession run.ezsession
```

Um número é pior quando cresceu mais do que a tolerância (10% por padrão). O relatório lista cada número como melhor, pior
ou igual. Sem o `--fail-on-regression`, a regressão só é reportada, que é uma boa forma de começar.

## O job do GitHub Actions

```yaml
name: UI tests
on: [pull_request]

jobs:
  android:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-java@v4
        with: { distribution: temurin, java-version: 17 }
      - name: Install EmuLabZip
        run: cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
      - uses: reactivecircus/android-emulator-runner@v2
        with:
          api-level: 34
          arch: x86_64
          script: >
            emulabzip ci -o run.ezsession --app com.example.app --fail-on-crash --screens 10
            -- ./gradlew connectedCheck
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: session
          path: run.ezsession
```

Baixe o artefato e abra no app desktop, ou leia com `emulabzip session show`. Para guardar a última sessão boa para a
baseline, armazene-a como cache ou como artefato da sua branch padrão e baixe antes da execução.

Dois outros checks cabem no mesmo job: `emulabzip a11y --fail-on error` barra a build por acessibilidade, e
`--fail-on-security warning` audita a segurança e a privacidade do app depois do comando.

## Observações

- O emulador precisa estar rodando antes de o `ci` começar, e com mais de um dispositivo você passa `--device`.
- Um arquivo de sessão que já existe nunca é sobrescrito.
- Com `--network`, o dispositivo precisa confiar no certificado do EmuLabZip: no CI, rode `emulabzip ca install` uma vez, em uma imagem com root.

## Experimente

- Modo CI: [emulabzip.luispeixoto.dev/pt/docs/ci/](https://emulabzip.luispeixoto.dev/pt/docs/ci/)
- Orçamentos e baselines: [emulabzip.luispeixoto.dev/pt/docs/budgets/](https://emulabzip.luispeixoto.dev/pt/docs/budgets/)
- Tempo de abertura do app: [emulabzip.luispeixoto.dev/pt/docs/startup/](https://emulabzip.luispeixoto.dev/pt/docs/startup/)
- Site e toda a documentação: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/pt/)
- Código-fonte (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
