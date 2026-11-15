---
source: 11355a8d3815
description: Analise um APK, AAB ou IPA para ver onde estão os bytes, compare dois builds e falhe o CI quando o tamanho passar do orçamento definido no emulabzip.toml.
---
# Tamanho do build

<p align="center">
  <img src="images/size-diff.png" alt="A view Build size comparando dois builds: o que cresceu, por tipo e por arquivo" />
</p>

`emulabzip size` desmonta um APK, um AAB ou um IPA e diz onde estão os bytes dele, `emulabzip size diff` diz o que mudou
entre dois builds, e um **orçamento de tamanho** no `emulabzip.toml` faz o CI falhar quando um build cresce demais. Ele lê o
arquivo neste computador: sem dispositivo, sem ferramenta do SDK, sem Xcode.

```bash
emulabzip size show app-release.apk
emulabzip size show app-release.aab --json
emulabzip size diff app-1.4.apk app-1.5.apk
emulabzip size diff app-1.4.apk app-1.5.apk --fail-on-growth 5      # exit 1 over 5% growth
emulabzip size show app-release.apk --budget --baseline app-1.4.apk # hold it to [size] in emulabzip.toml
```

## O que você recebe

- O tamanho de download (comprimido, como armazenado no arquivo) e o tamanho instalado, por **tipo**: `dex` (código
  compilado), `native` (`lib/<abi>/*.so` e, no iOS, o executável e `Frameworks/`), `resources`, `assets`, `manifest`,
  `signing` e `other`.
- O código nativo **por ABI** (`arm64-v8a`, `x86_64`…), para que um build que entrega quatro delas fique evidente. Em um
  IPA, `Frameworks` e o executável.
- Cada **arquivo dex** com suas classes e métodos, lidos do cabeçalho dele, e o total contra os 65.536 métodos que um único
  arquivo dex comporta.
- Os **módulos** de um AAB (`base` e os módulos de feature).
- Os maiores arquivos.
- Com `diff`: o total e a porcentagem, a mudança por tipo e por pasta, e os maiores arquivos **adicionados, removidos e
  alterados**.

Um arquivo é identificado como APK, AAB ou IPA pelo que há dentro dele, não pelo nome.

## O orçamento

```toml
[size]
max_total_mb = 25          # the download size, compressed
max_dex_mb = 8
max_native_mb = 12
max_resources_mb = 6
max_methods = 60000        # all dex files together
max_growth_percent = 5     # against a baseline build (--baseline, or ci --size-baseline)
```

Os tamanhos são megabytes (1.048.576 bytes) de tamanho **comprimido**. Toda chave é opcional, uma chave desconhecida é um
erro, e um limite de crescimento sem um build de baseline é reportado em vez de aprovado. No CI:

```bash
emulabzip ci -o run.ezsession --size app-release.apk --size-baseline last-release.apk -- ./gradlew connectedCheck
```

O build é lido antes de qualquer coisa rodar, então um arquivo ausente não custa uma execução de teste. Veja
[Orçamentos](budgets.md).

## No desktop

A view **Build size** abre um build com uma janela de arquivo (sem precisar de dispositivo), mostra as barras por tipo, o
código nativo, os arquivos dex e os maiores arquivos, e **Compare with…** um segundo build.

## Limites

- Ele não abre o manifest compilado nem o bytecode dex: apenas tamanhos, pastas e o cabeçalho do dex. Para um detalhamento
  por classe, use o APK Analyzer do Android Studio.
- Ele ainda não lê o app instalado a partir de um dispositivo: puxe o arquivo antes (`adb shell pm path`, `adb pull`).
- Por MCP (`analyze_package`, `diff_packages`) um arquivo só é aceito com a extensão `.apk`, `.aab` ou `.ipa`.
