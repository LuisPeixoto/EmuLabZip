---
source: 0f80060ba988
description: Crie, inicie, pare, apague e exclua emuladores e simuladores pela view Devices ou pela linha de comando, e mantenha os snapshots de um emulador.
---
# Dispositivos: emuladores e simuladores

<p align="center">
  <img src="images/devices-manage.png" alt="A view Devices: emuladores em execução ou não, com Start, Headless, Wipe e Delete" />
</p>

A view **Devices** e os comandos `avd`, `sim` e `snapshot` criam, iniciam, param, apagam e excluem emuladores e
simuladores, e mantêm os snapshots de um emulador. Até agora o EmuLabZip só enxergava dispositivos que já estavam em
execução.

```bash
emulabzip devices --all                       # running or not; a stopped one has the state "shutdown"
emulabzip avd options                         # system images and device definitions
emulabzip avd create Pixel_Test --image "system-images;android-35;google_apis;arm64-v8a" --model pixel_6
emulabzip avd boot Pixel_Test --headless
emulabzip snapshot save before-login -d emulator-5554
emulabzip snapshot load before-login -d emulator-5554 --yes
emulabzip avd shutdown Pixel_Test
emulabzip avd wipe Pixel_Test --yes
emulabzip avd delete Pixel_Test --yes
```

O `sim` é o equivalente para o iOS Simulator (`sim options` lista runtimes e tipos de dispositivo; `sim wipe` também é
`sim erase`).

## O que exige `--yes`

Tudo o que descarta dados: `wipe` (`erase`), `delete`, `boot --wipe` e `snapshot load` e `delete`. O comando diz o que
precisa antes de procurar qualquer dispositivo. O app desktop pergunta em uma caixa de diálogo. Iniciar, parar, criar e
salvar um snapshot não precisam de nada.

Um dispositivo em execução não é excluído, e um emulador Android não é apagado enquanto está rodando: desligue-o antes.

## Android

| | Como |
|---|-----|
| Listar | `~/.android/avd/*.ini` (ou `ANDROID_AVD_HOME`) combinado com o que o `adb` enxerga; um emulador que não está em execução tem o id `avd:<name>` |
| Opções | As imagens de sistema instaladas no SDK e `avdmanager list device` |
| Criar, excluir | `avdmanager` |
| Iniciar | `emulator -avd <name>`, desanexado, até `sys.boot_completed`; `--headless` não tem janela, `--cold` ignora o snapshot de quick boot, `--wipe` inicia a partir do estado de fábrica |
| Parar | O console do emulador (`kill`) |
| Apagar | Exclui o `userdata-qemu.img`, o cache e os snapshots do AVD, de modo que ele inicie do estado de fábrica na próxima vez (o emulador precisa estar parado); `boot --wipe` inicia com `-wipe-data` |
| Snapshots | O console: `avd snapshot list`, `save`, `load`, `del` |

O SDK é encontrado a partir de `ANDROID_HOME`, `ANDROID_SDK_ROOT` e das pastas usuais. Um emulador em execução informa
o nome com os sublinhados trocados por espaços; o EmuLabZip faz a correspondência entre os dois.

## iOS Simulator

`simctl create`, `boot`, `shutdown`, `erase` e `delete`, com os identificadores de runtime e de tipo de dispositivo de
`simctl list`. O `delete` de um simulador inicializado é recusado aqui (o próprio `simctl` permitiria).

## Nomes

Um nome é formado por letras, dígitos, `.`, `_` e `-`, com até 64 caracteres, de modo que nunca possa levar nada ao
`avdmanager`, ao `emulator` ou ao `simctl`. Imagens e runtimes também permitem `;` e `:`.

## Para assistentes

Via [MCP](mcp.md): `list_all_devices`, `boot_device`, `shutdown_device`, `list_emulator_snapshots` e
`save_emulator_snapshot`. Iniciar nunca apaga. Apagar, excluir e carregar um snapshot **não** são oferecidos.
