---
title: Criei um laboratório de debug open source para emuladores Android e simuladores iOS
description: O EmuLabZip reúne logs, rede, desempenho, memória, armazenamento e crashes dos seus emuladores e simuladores em um app Rust rápido, com CLI e servidor MCP.
tags: android, ios, rust, opensource
series: EmuLabZip (PT)
cover: docs/images/overview.png
publish_after: 2026-11-15
pair: 01-introducing
lang: pt
---

Quando depuro um app mobile, acabo com seis ferramentas abertas: o Logcat, a janela do Instruments, os profilers do Android
Studio, um proxy como o Charles, um terminal cheio de `adb shell` e `xcrun simctl`, e um editor de texto para ler algum
banco de dados que puxei do dispositivo. Cada uma conhece uma fatia do que o emulador sabe, e nenhuma conversa com as outras.

Por isso criei o **EmuLabZip**: um app open source que se conecta aos seus emuladores Android e simuladores iOS em execução
e traz tudo isso para uma janela só. Ele é escrito em Rust, vem como app desktop (Tauri + Svelte) e como ferramenta de linha
de comando, e nada é instalado no dispositivo.

![A visão geral de um emulador Android no EmuLabZip](docs/images/overview.png)

## O que ele faz

Você inicia um emulador ou liga um simulador, e ele aparece. Não há configuração no dispositivo e nada para adicionar ao seu
app. A partir daí:

- **Logs** do logcat e do unified log do iOS em uma única tabela que continua fluida com milhares de linhas por segundo, com
  filtros por nível, tag, texto, regex e por app.
- **Desempenho e memória**: CPU, memória, taxa de quadros, frames travados e bateria do app que você escolher, snapshots de
  memória que você compara por categoria, e heap dumps do Android com um detector de vazamentos embutido.
- **Network**: um proxy de debug que decifra HTTPS, com mocks, breakpoints, limitação de banda, e decodificação de GraphQL e
  gRPC.
- **Storage**: navegue pelos arquivos de um app e abra SQLite, SharedPreferences, property lists, MMKV, Realm e mais, ali
  mesmo.
- **Crashes**: crashes, crashes nativos e ANRs com suas stacks, legíveis quando você fornece o `mapping.txt` do R8, o source
  map ou o dSYM.
- **Sessions**: grave uma execução inteira (logs, requisições, crashes, vídeo da tela, toques) em um único arquivo que você
  pode abrir depois, comparar ou coletar no CI.

![Requisições ao vivo de um emulador Android, com o protocolo usado por cada uma](docs/images/network.png)

## Por que Rust, e por que sem agente no dispositivo

O EmuLabZip não injeta nada no seu app. Ele usa as interfaces públicas de cada plataforma: no Android, tem um cliente async
próprio para o protocolo host do ADB (sem precisar chamar o `adb` a cada operação), e no iOS controla o `xcrun simctl`. O
código de Android e o de Apple ficam em crates separados, atrás de uma única trait `Backend`, então o app desktop e a CLI
usam exatamente o mesmo código.

Isso também significa que ele continua funcionando quando o Xcode ou o Android Studio atualizam, porque depende de
interfaces que eles precisam manter.

## Os mesmos dados no terminal

Tudo o que o app mostra também é um comando. Inicie um dispositivo e rode:

```bash
emulabzip devices
emulabzip logs
emulabzip mem com.example.app
emulabzip net --har capture.har
emulabzip crashes --app com.example.app
```

Cada comando imprime tabelas para pessoas e `--json` para scripts. O mesmo motor roda no CI: `emulabzip ci` grava uma sessão
em volta do seu comando de teste e sai com o mesmo código de saída dele, então o pipeline se comporta como antes e você ganha
um arquivo para abrir quando algo falha. Vou escrever sobre isso em um próximo post.

## E conversa com assistentes de IA

O EmuLabZip inclui um servidor MCP (`emulabzip mcp`). Conecte o Claude, o Cursor ou o VS Code e o assistente consegue ler
seus logs e crashes, acompanhar o tráfego ao vivo, controlar a tela e gravar uma sessão. Ele não consegue apagar nem
reescrever nada, a menos que você inicie o servidor com `--allow-writes`, e aí cada alteração pede sua confirmação antes.
Também vai ter um post sobre isso.

## Experimente

O app desktop tem instaladores para macOS, Windows e Linux. A ferramenta de linha de comando se instala com o Cargo:

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

Depois inicie um emulador ou simulador e rode `emulabzip devices`.

- Site e documentação: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/pt/)
- Guia de primeiros passos: [emulabzip.luispeixoto.dev/pt/docs/getting-started/](https://emulabzip.luispeixoto.dev/pt/docs/getting-started/)
- Código-fonte e releases (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)

Se algo quebrar ou estiver faltando, me avise em uma issue. É um projeto de uma pessoa só e o roadmap segue o que as pessoas
precisam.
