---
title: Crashes ofuscados legíveis: mappings do R8, source maps do React Native e dSYMs
description: Leia stacks de crash de Android, React Native e iOS com o mapping, source map ou dSYM do build exato e junte um relatório de bug em um HTML, com o EmuLabZip.
tags: android, ios, reactnative, debugging
series: EmuLabZip (PT)
cover: docs/images/symbolication.png
publish_after: 2026-11-24
pair: 04-readable-crashes
lang: pt
---

Um build de release esconde os nomes. O R8 e o ProGuard renomeiam suas classes para `a.b.c`. O React Native entrega um único
bundle JavaScript em que toda posição é `1:608617`. Um app iOS sem símbolos só tem endereços soltos. O crash que chega na sua
caixa de entrada quase não diz nada, a não ser que você tenha o arquivo que o build gerou justamente para isso e uma
ferramenta que o aplique.

Este post mostra como o EmuLabZip lista os crashes que o seu emulador ou simulador registrou, traduz cada um de volta para o
que você escreveu e transforma um deles em um relatório de bug com um único comando.

![O mesmo crash Android, legível com o mapping.txt e como o dispositivo o escreveu](docs/images/symbolication.png)

## Encontre o crash

Crashes, crashes nativos e ANRs são coletados pelo próprio dispositivo. O EmuLabZip lista todos, do mais recente para o mais
antigo, cada um com um id:

```bash
emulabzip crashes --app com.example.shop
emulabzip crash dropbox:data_app_crash:20261023205928
```

Cada crash abre com a stack, os fatos que o dispositivo anotou e dicas do que tentar em seguida. Um ANR mostra a stack de todas
as threads, então dá para ver o que bloqueou a principal. O `--all-threads` faz o mesmo para os outros crashes, e o `--raw`
mostra o relatório exatamente como o dispositivo o escreveu. No app desktop, abra **Crashes**: os crashes novos aparecem
sozinhos enquanto ele está aberto, com uma notificação.

## Guarde o arquivo do build

Um mapping só está certo para o build que o gerou, e ler outro build com ele é pior do que não ter mapping nenhum. Por isso o
EmuLabZip guarda os arquivos por app e versão e só os aplica a um crash da mesma versão. Você adiciona o arquivo uma vez:

```bash
emulabzip mappings add com.example.shop "1.2.0 (42)" app/build/outputs/mapping/release/mapping.txt
emulabzip crash dropbox:data_app_crash:20261023205928
```

A partir daí, todo crash desse build fica legível. A versão é a que o crash mostra: nome e código da versão no Android
(`1.2.0 (42)`), versão curta e número do build no iOS. Quando nenhum arquivo serve, o crash avisa e diz qual versão ele
precisa. Os arquivos são copiados para a pasta de dados do EmuLabZip, então um clean build não os leva embora.

O que você recebe depende da stack:

| O que quebrou | O arquivo | O que você recebe |
|---|---|---|
| Java ou Kotlin, minificado com R8 ou ProGuard | `mapping.txt` | `Cart.average` em `MainActivity.kt:39`, com as chamadas inlined devolvidas como frames próprios |
| JavaScript do React Native (Hermes ou JavaScriptCore) | o source map do bundle | `App.tsx 17:14` em vez de `1:608617` |
| Um app iOS | o `.dSYM` dele | função, arquivo e linha para cada endereço (em um Mac, usando `atos`) |

O R8 grava o `mapping.txt` em `app/build/outputs/mapping/<variant>/` quando `isMinifyEnabled` está ligado. Guarde-o de cada
release que você publicar: é o único caminho de volta a partir dos nomes de um crash.

## Leia uma stack que mandaram para você

Usuários e logs de CI entregam stacks coladas, não crashes ao vivo. Passe-as por pipe:

```bash
emulabzip symbolicate stack.txt --mapping mapping.txt
adb logcat -d | emulabzip symbolicate - --mapping mapping.txt
```

Ou indique o arquivo para um único crash sem guardá-lo: `emulabzip crash <id> --mapping app/build/outputs/mapping/release/mapping.txt`.

## Um relatório de bug que outra pessoa consegue ler

Um relatório de bug precisa sempre das mesmas coisas: como estava a tela, qual dispositivo e app, o que o log dizia, quais
requisições falharam e o crash legível. O EmuLabZip junta tudo em um único arquivo HTML:

```bash
emulabzip report --app com.example.shop -o report.html
emulabzip report --app com.example.shop --issue github:owner/repo --open
emulabzip report --session run.ezsession -o report.html
```

O segundo comando também abre uma issue nova com um resumo já preenchido, e o terceiro monta o relatório a partir de uma
sessão gravada, com o trecho em volta do crash. Nada é enviado para lugar nenhum: você vê o arquivo e decide para onde ele
vai. No app, é o botão **Report a bug** na barra de ferramentas.

![O diálogo Report a bug](docs/images/bug-report-dialog.png)

## Experimente

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

- Guia de crashes: [emulabzip.luispeixoto.dev/pt/docs/crashes/](https://emulabzip.luispeixoto.dev/pt/docs/crashes/)
- Stacks legíveis: [emulabzip.luispeixoto.dev/pt/docs/symbolication/](https://emulabzip.luispeixoto.dev/pt/docs/symbolication/)
- Relatórios de bug: [emulabzip.luispeixoto.dev/pt/docs/bug-reports/](https://emulabzip.luispeixoto.dev/pt/docs/bug-reports/)
- Site e toda a documentação: [emulabzip.luispeixoto.dev/pt/](https://emulabzip.luispeixoto.dev/pt/)
- Código-fonte (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
