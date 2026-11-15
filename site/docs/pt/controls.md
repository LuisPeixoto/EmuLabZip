---
source: 90122b673163
description: "A ferramenta Controls muda o que um dispositivo informa ou faz: localização, tema, barra de status, bateria, permissões, notificações e links, tudo reversível."
---
# Controls

A ferramenta Controls muda o que um dispositivo informa ou faz, para você testar um app em outro lugar, no escuro, com uma
barra de status limpa ou uma permissão a menos, sem mexer nas telas de configurações do próprio dispositivo. Toda mudança pode
ser desfeita no mesmo card.

| Controle | Android | iOS Simulator |
|---------|---------|---------------|
| **Location** | Só emuladores, pelo console do emulador | ✅ |
| **Appearance** (claro ou escuro) | Android 10 e posterior | ✅ |
| **Status bar** (hora, bateria, Wi-Fi, celular) | Demo mode do SystemUI | ✅ |
| **Battery** (nível, carregando) | ✅ | não é possível: use a status bar |
| **Permissions** | Concede e revoga qualquer permissão de runtime do app escolhido, mostrando o estado atual | Concede, revoga ou pergunta de novo para um serviço (fotos, localização, …) |
| **Notification** | Publicada pelo shell | Um push simulado para o app escolhido, com seu próprio payload se quiser |
| **Open a link** | ✅ | ✅ |

<p align="center">
  <img src="images/controls.png" alt="A tela Controls" />
</p>

## Primeiros passos

Abra **Controls** (<kbd>⌘9</kbd>). Só aparecem os cards que o dispositivo selecionado suporta. O card de permissões segue o app
escolhido na barra de ferramentas. Depois de cada mudança o dispositivo é lido de volta, então a página mostra o que ele
realmente informa.

Em um terminal:

```bash
emulabzip control appearance dark
emulabzip control location set -23.5505 -46.6333       # São Paulo
emulabzip control location clear
emulabzip control statusbar set --time 9:41 --battery 100
emulabzip control statusbar clear
emulabzip control permission com.example.app android.permission.CAMERA revoke
emulabzip control permission com.example.app photos reset      # iOS
emulabzip control notify com.example.app --title Hello --body "A test"
emulabzip control open myapp://items/3
emulabzip control battery set 15
emulabzip control battery reset
emulabzip control info --app com.example.app                    # what is supported, and the permissions
```

## O que cada um faz no dispositivo

**Android.** Tudo passa por `cmd`, `settings`, `pm`, `am` e `dumpsys`, que o shell pode executar, e pelo console do emulador
no caso da localização.

- *Location* envia `geo fix` ao console em `localhost:<port>`, com o token em `~/.emulator_console_auth_token`. O emulador não
  tem como esquecer uma posição, então **Reset** a devolve ao padrão dele (o Googleplex). Dispositivos físicos não são
  suportados.
- *Appearance* é `cmd uimode night yes|no`, e é lido de volta.
- *Status bar* entra no demo mode do SystemUI (`sysui_demo_allowed` é definido como 1 e fica assim) e mostra o que você pediu,
  com os ícones de notificação ocultos. **Clear** sai do demo mode.
- *Battery* é `dumpsys battery set level`, com `set ac 1` para carregando e `unplug` caso contrário. **Reset** é
  `dumpsys battery reset`.
- *Notification* é `cmd notification post`. O Android a publica como quem executa o comando, então quando o `adbd` é root (os
  emuladores sem Google Play) o EmuLabZip a executa como o usuário shell. O argumento do app é ignorado.

- *Text size* é `settings put system font_scale`, como porcentagem do normal (50 a 300).
- *Language* é `cmd locale set-app-locales` para **um app** (Android 13 e posterior), incluindo as pseudo-locales `en-XA` e
  `ar-XB`. *Display size* é `wm size` e `wm density`. Os três podem ser restaurados. Veja
  [Idioma, tamanho do texto e texto que não cabe](localization.md).

- *Touch, swipe, text and keys* (toque, deslize, texto e teclas) são `input tap`, `input swipe`, `input text` e
  `input keyevent`; *Launch* é `monkey -p` com a categoria do launcher; *Verify links* é `pm verify-app-links --re-verify`.
  Veja [Entrada e fluxos](automation.md) e [Deep links](deep-links.md).

**iOS Simulator.** Tudo é `xcrun simctl`: `location`, `ui appearance`, `status_bar`, `privacy`, `push` e `openurl`, com
`ui content_size` para o tamanho do texto e os defaults globais (`AppleLanguages`, `AppleLocale`) para o idioma, que o app adota quando é iniciado de novo. O iOS não informa quais permissões um app tem, então o card oferece só os serviços e as ações. *Launch* é `simctl launch`;
toques, deslizes, texto e teclas não são oferecidos no iOS, que não tem um modo público de enviá-los.

## Segurança

Todo control é verificado antes de chegar a um dispositivo, venha ele do app, da linha de comando ou de um assistente: as
coordenadas devem ser números dentro do intervalo, a hora deve parecer uma hora, os nomes só podem usar caracteres simples e
um payload deve ser JSON de no máximo 4 KB. **Links** podem usar `http`, `https` ou o esquema próprio de um app; `file:`,
`javascript:`, `data:`, `content:`, `intent:` e similares são recusados. Valores que chegam ao shell do Android são colocados
entre aspas, e o `simctl` é iniciado sem um shell.
