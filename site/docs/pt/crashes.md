---
source: fe2e62872ca2
description: A ferramenta Crashes lista crashes, crashes nativos e ANRs de um dispositivo, com stack, detalhes e dicas do que tentar, e avisa quando surge um novo.
---
# Crashes

A ferramenta Crashes lista os crashes, os crashes nativos e os ANRs que um dispositivo registrou, e abre cada um com a
sua stack, os fatos que o dispositivo anotou e o que tentar em seguida. Os novos aparecem sozinhos, com uma notificação
e um badge na barra lateral, enquanto o EmuLabZip está aberto.

| Tipo | Android | iOS Simulator |
|------|---------|---------------|
| **Crash** | Uma exceção não tratada, com a sua cadeia de `Caused by:` | Uma falha de runtime do Swift, um `abort()` ou uma `NSException` não tratada |
| **Native crash** | Um sinal fatal, vindo do tombstone | Um acesso inválido à memória (`EXC_BAD_ACCESS`) e similares |
| **ANR** | Application Not Responding, com a stack de cada thread | não se aplica |

<p align="center">
  <img src="images/crashes.png" alt="Um ANR com a thread principal bloqueada, dicas e detalhes" />
</p>

## Início rápido

1. Abra **Crashes** (<kbd>⌘8</kbd>). O crash mais recente abre sozinho.
2. Escolha outro à esquerda, ou filtre por tipo, ou pelo app escolhido na barra de ferramentas.
3. **Report** mostra as dicas, os detalhes e as threads. **Raw** mostra o relatório como o dispositivo o escreveu.
   **Save…** grava-o em um arquivo.
4. Um build de release embaralha os seus nomes. Adicione o `mapping.txt`, o source map ou o dSYM dele quando o crash
   pedir, e o crash é lido de novo, com **Readable** e **Original** para alternar entre os dois. Veja
   [Stacks legíveis](symbolication.md).
5. **Report a bug**, na barra de ferramentas, reúne o crash, um screenshot, o log e as requisições com falha em um único
   arquivo. Veja [Relatórios de bug](bug-reports.md).

Em um terminal:

```bash
emulabzip crashes --app com.example.app          # newest first, with an id for each
emulabzip crash dropbox:data_app_crash:20261023205928
emulabzip crash <id> --all-threads               # every thread, not only the one that crashed
emulabzip crash <id> --raw                       # as the device wrote it
emulabzip crash <id> --mapping mapping.txt       # readable, with a file you name
```

## De onde vêm os relatórios

**Android.** O sistema guarda uma cópia de cada crash, ANR e tombstone no DropBox, e o shell pode lê-la com
`dumpsys dropbox --print <tag>`. Isso significa que funciona em qualquer dispositivo e em qualquer emulador, sem root e
sem nada instalado. O EmuLabZip lê `data_app_crash`, `system_app_crash`, `data_app_native_crash`, `SYSTEM_TOMBSTONE`,
`data_app_anr` e `system_app_anr`. Os ids são parecidos com `dropbox:data_app_crash:20261023205928`. O sistema limita a
taxa de crashes repetidos de um mesmo app, então um crash em loop aparece listado menos vezes do que acontece.

**iOS Simulator.** O macOS grava um relatório `.ips` em `~/Library/Logs/DiagnosticReports` quando um processo quebra,
inclusive os apps simulados. Um relatório pertence a um simulador quando o caminho do app passa por
`CoreSimulator/Devices/<udid>`. O EmuLabZip lê os 200 arquivos mais recentes ali e em `Retired`. Os ids são parecidos
com `ips:MyApp-2026-10-23-210246.ips`. O macOS grava o relatório alguns segundos depois do crash.

## Lendo um relatório

A thread que quebrou fica aberta; as outras ficam recolhidas. Em um ANR, a thread principal bloqueada vem primeiro.
Cada frame tem um quadrado colorido que indica de quem é o código, as mesmas quatro cores do flame graph:

| Cor | Significado |
|--------|---------|
| Azul | O código do próprio app |
| Verde | A plataforma: o framework e o runtime do Android, UIKit, Foundation, o runtime do Swift |
| Laranja | Bibliotecas nativas |
| Cinza | O kernel |

**Only the app's code** esconde todo o resto, que é a forma de ler uma stack nativa composta em sua maior parte por
internos do runtime.

Um crash de Java mostra uma seção por exceção da cadeia, e a última, a causa raiz, é a marcada como a que quebrou. O
título do crash é essa causa raiz.

## Dicas

O EmuLabZip diz o que consegue deduzir do relatório:

- Nomes de classe que parecem ofuscados: adicione o `mapping.txt` do build e eles são lidos para você. Veja
  [Stacks legíveis](symbolication.md).
- Posições de JavaScript de um app React Native: adicione o source map do bundle.
- Frames nativos do app sem símbolos: `ndk-stack -sym <folder> -dump tombstone.txt`, ou `llvm-symbolizer`. O BuildId
  precisa corresponder ao build que quebrou.
- Frames de iOS do app sem símbolos: adicione o dSYM do build e eles são lidos com `atos`, ou execute o comando `atos`
  mostrado, com a arquitetura e o endereço de carga preenchidos.
- Um ANR cuja thread principal estava dormindo ou esperando: a thread principal estava bloqueada.
- Um ANR em um dispositivo com falta de CPU: o app pode não ser o culpado.
- Um crash nativo cujo sinal veio de `kill()`: ele foi enviado, não gerado por uma falha.

## Experimente

`tools/crash-app` (Android) e `tools/crash-app-ios` (iOS Simulator) são apps minúsculos que falham de propósito, para
você ver cada tipo sem precisar escrever um. Cada um tem um `install.sh` e um README.
