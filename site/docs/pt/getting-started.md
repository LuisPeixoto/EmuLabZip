---
source: 76f23448cd4b
description: "Do primeiro uso à leitura dos logs do seu app em poucos minutos: instale o EmuLabZip, prepare as ferramentas da plataforma, inicie um dispositivo e explore."
---
# Primeiros passos

Este guia leva você de uma instalação nova até a leitura dos logs do seu app em poucos minutos.

## 1. Instale o EmuLabZip

Baixe o instalador para o seu sistema na [última release](https://github.com/LuisPeixoto/EmuLabZip/releases),
ou [compile a partir do código-fonte](../README.md#build-from-source).

## 2. Deixe as ferramentas da plataforma prontas

| Para inspecionar | Você precisa de |
|------------|----------|
| Emuladores e dispositivos Android | As **platform tools** (`adb`) do Android SDK. O Android Studio as instala. |
| Simuladores iOS | Um Mac com o **Xcode** e suas ferramentas de linha de comando (`xcode-select --install`). |

O EmuLabZip encontra o `adb` no seu `PATH`, em `$ANDROID_HOME` / `$ANDROID_SDK_ROOT` ou no local padrão do SDK
(`~/Library/Android/sdk`, `%LOCALAPPDATA%\Android\Sdk`, `~/Android/Sdk`). Ele inicia o servidor ADB se ele não estiver
em execução. Veja [Android](android.md) e [iOS Simulator](ios-simulator.md) para os detalhes.

## 3. Inicie um dispositivo

```bash
# Android
emulator -list-avds
emulator -avd Pixel_9

# iOS
xcrun simctl boot "iPhone 17"
open -a Simulator
```

Abra o EmuLabZip. Os dispositivos em execução aparecem no **device picker**, no canto superior esquerdo, em poucos
segundos, e o primeiro que estiver pronto é selecionado automaticamente.

## 4. Dê uma olhada

- **Overview** mostra o que o dispositivo é: SO e build, hardware, tela, bateria, armazenamento e idioma, com uma
  prévia ao vivo da tela. Clique em qualquer valor para copiá-lo.
- **Logs** acompanha o log do dispositivo desde o momento em que ele é selecionado, então o histórico já está lá
  quando você abre a aba. Veja [Logs](logs.md).
- **Performance** exibe CPU, memória, taxa de quadros e bateria ao vivo em gráficos. Veja [Performance](performance.md).
- **Memory** tira snapshots, compara e salva heap dumps. Veja [Memory](memory.md).
- **Storage** navega pelos arquivos do app e abre bancos de dados, preferências e property lists. Veja
  [Storage](storage.md).
- **Network** captura o tráfego HTTP e HTTPS do dispositivo. Veja [Network](network.md).
- **Traces** registra o que o sistema e um app fizeram e mostra para onde o tempo foi. Veja [Traces](traces.md).
- **Crashes** lista crashes, crashes nativos e ANRs com suas stacks, e avisa quando um novo acontece. Veja
  [Crashes](crashes.md).
- **Controls** muda a localização, a aparência, a barra de status, a bateria e as permissões do dispositivo. Veja
  [Controls](controls.md).
- **Plugins** executa os seus próprios coletores. Veja [Plugins](plugins.md). Para deixar um assistente de IA usar
  seus dispositivos, veja o [servidor MCP](mcp.md).
- **Screenshot**, na barra de ferramentas, captura a tela e permite salvá-la como PNG.

## 5. Foque no seu app

Escolha o seu app no **app picker**, ao lado do dispositivo. Depois, em Logs, ative o filtro de app para ver só as
linhas que o processo dele escreveu. O EmuLabZip acompanha o id do processo, então o filtro continua funcionando
quando o app reinicia.

## Atalhos de teclado

| Atalho | Ação |
|----------|--------|
| <kbd>⌘1</kbd> … <kbd>⌘6</kbd> | Overview, Logs, Performance, Memory, Storage, Network |
| <kbd>⌘F</kbd> | Pesquisar no log |
| <kbd>⌘K</kbd> | Limpar o log |
| <kbd>⌘R</kbd> | Procurar dispositivos agora |
| <kbd>⌘,</kbd> | Settings |

No Windows e no Linux, use <kbd>Ctrl</kbd> no lugar de <kbd>⌘</kbd>.
