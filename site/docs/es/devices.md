---
source: 0f80060ba988
description: Crea, arranca, detén, borra y limpia emuladores y simuladores desde la vista Devices o la CLI, y guarda snapshots de un emulador.
---
# Dispositivos: emuladores y simuladores

<p align="center">
  <img src="images/devices-manage.png" alt="La vista Devices: emuladores en ejecución o no, con Start, Headless, Wipe y Delete" />
</p>

La vista **Devices** y los comandos `avd`, `sim` y `snapshot` crean, arrancan, detienen, limpian y borran emuladores y
simuladores, y guardan los snapshots de un emulador. Hasta ahora EmuLabZip solo veía los dispositivos que ya estaban en
ejecución.

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

`sim` es lo mismo para el iOS Simulator (`sim options` lista runtimes y tipos de dispositivo; `sim wipe` también es `sim erase`).

## Qué necesita `--yes`

Todo lo que tira datos: `wipe` (`erase`), `delete`, `boot --wipe` y `snapshot load` y `delete`. El comando dice lo que
necesita antes de buscar ningún dispositivo. La app de escritorio lo pregunta en un diálogo. Arrancar, detener, crear y
guardar un snapshot no necesitan nada.

Un dispositivo en ejecución no se borra, y un emulador Android no se limpia mientras se ejecuta: apágalo primero.

## Android

| | Cómo |
|---|-----|
| Listar | `~/.android/avd/*.ini` (o `ANDROID_AVD_HOME`) unido con lo que ve `adb`; un emulador que no está en ejecución tiene el id `avd:<name>` |
| Opciones | Las imágenes del sistema instaladas en el SDK y `avdmanager list device` |
| Crear, borrar | `avdmanager` |
| Arrancar | `emulator -avd <name>`, desacoplado, hasta `sys.boot_completed`; `--headless` no tiene ventana, `--cold` omite el snapshot de arranque rápido, `--wipe` arranca desde el estado de fábrica |
| Detener | La consola del emulador (`kill`) |
| Limpiar | Borra el `userdata-qemu.img` del AVD, la caché y los snapshots, así que la próxima vez arranca desde el estado de fábrica (el emulador debe estar detenido); `boot --wipe` arranca con `-wipe-data` |
| Snapshots | La consola: `avd snapshot list`, `save`, `load`, `del` |

El SDK se encuentra a partir de `ANDROID_HOME`, `ANDROID_SDK_ROOT` y las carpetas habituales. Un emulador en ejecución
informa su nombre con los guiones bajos convertidos en espacios; EmuLabZip hace coincidir ambos.

## iOS Simulator

`simctl create`, `boot`, `shutdown`, `erase` y `delete`, con los identificadores de runtime y de tipo de dispositivo de
`simctl list`. Aquí se rechaza `delete` de un simulador arrancado (`simctl` en sí lo
permitiría).

## Nombres

Un nombre son letras, dígitos, `.`, `_` y `-`, hasta 64 caracteres, así que nunca puede llevar nada a `avdmanager`,
`emulator` o `simctl`. Las imágenes y los runtimes también admiten `;` y `:`.

## Para asistentes

Por [MCP](mcp.md): `list_all_devices`, `boot_device`, `shutdown_device`, `list_emulator_snapshots` y
`save_emulator_snapshot`. Arrancar nunca limpia. Limpiar, borrar y cargar un snapshot **no** se ofrecen.
