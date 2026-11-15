---
source: 9f0d25bee3d4
description: Grave uma sessão em volta de qualquer comando de teste com emulabzip ci, mantendo o código de saída do comando e falhando por crash, orçamento ou regressão.
---
# Modo CI

`emulabzip ci` grava uma [sessão](sessions.md) em volta de qualquer comando, normalmente uma execução de testes, e depois sai
com o código de saída do próprio comando, então o seu pipeline se comporta como antes, com uma gravação para examinar
quando algo dá errado.

```bash
emulabzip ci -o run.ezsession --app com.example.app -- ./gradlew connectedCheck
```

Ele começa a gravar, executa o comando (a saída dele passa sem alteração), para, escreve a sessão e imprime um resultado de
uma linha.

| Opção | |
|--------|---|
| `-o, --output FILE` | Onde escrever a sessão (obrigatório) |
| `-a, --app ID` | O app: as amostras são tiradas para ele e os crashes são limitados a ele |
| `--network` | Grava o tráfego HTTP(S): o dispositivo aponta para o proxy durante a execução |
| `--screens N` | Um screenshot a cada N segundos |
| `--fail-on-crash` | Falha a execução (saída 1) quando o app deu crash enquanto rodava, mesmo que o comando tenha passado |
| `--summary FILE` | Também escreve o relatório, em Markdown, neste arquivo |
| `--config FILE` | Limites a que a execução será submetida (padrão: `./emulabzip.toml`, quando existe). Veja [Orçamentos](budgets.md) |
| `--baseline FILE` | Uma sessão anterior para comparar |
| `--fail-on-regression` | Falha quando um número ficou pior do que o da baseline por mais do que a tolerância |
| `--video [N]` | Grava a tela, em pedaços de N segundos (padrão 30). Veja [Linha do tempo e vídeo da tela](timeline.md) |
| `--taps` | Grava toques (Android) |
| `--replay FILE` | Responde às requisições do app a partir de um arquivo HAR ou de uma sessão, para que a execução não dependa de nenhum servidor. Precisa de `--network`. Veja [Reproduzindo uma gravação](replay.md) |
| `--passthrough` | Com `--replay`: deixa as requisições que não estão na gravação chegarem ao servidor real |
| `--fail-on-miss` | Com `--replay`: falha a execução quando o app pediu algo que a gravação não tem |
| `--size BUILD` | Submete um APK, AAB ou IPA aos limites `[size]` do `emulabzip.toml`. Lido antes da execução. Veja [Tamanho do build](package-size.md) |
| `--size-baseline BUILD` | Com `--size`: um build anterior, para medir o crescimento |
| `--fail-on-security LEVEL` | Audita a segurança e a privacidade do app depois do comando (precisa de `--app`; as requisições do proxy são incluídas) e falha com um achado pelo menos tão grave quanto este: `note`, `warning` ou `error`. Veja [Auditoria de segurança](security-audit.md) |
| `--visual NAME` | Compara a tela depois do comando com uma baseline. Veja [Regressão visual](visual-regression.md) |
| `--fail-on-visual` | Com `--visual`: falha a execução quando a tela difere mais do que o limite |

O código de saída é o do comando. Se o comando passou mas `--fail-on-crash` encontrou um crash, um [orçamento](budgets.md)
foi estourado ou foi pedido que uma regressão falhasse a execução, ele é 1; se você pressionar Ctrl+C, ele é 130.

Quando a
variável de ambiente `GITHUB_STEP_SUMMARY` está definida (GitHub Actions), o relatório em Markdown é acrescentado a ela e
aparece na página da execução sem passos extras.

## GitHub Actions

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

Baixe o artefato e abra-o em **Sessions → Open…**, ou leia-o com `emulabzip session show`.

Você também pode barrar por acessibilidade no mesmo job, depois que o app estiver na tela em que deve ser verificado:

```bash
emulabzip a11y --fail-on error
```

## Notas

- O emulador precisa estar rodando antes de o `ci` começar; com mais de um dispositivo, passe `--device`.
- Um arquivo de sessão que já existe nunca é sobrescrito.
- Se `--network` for usado, o dispositivo precisa confiar no certificado do EmuLabZip (veja
  [Network](network.md#the-certificate)). No CI, execute `emulabzip ca install` uma vez, em uma imagem com root possível.
