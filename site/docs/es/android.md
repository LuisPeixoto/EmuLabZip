---
source: 854d8470ae63
description: Cómo EmuLabZip se conecta a emuladores y dispositivos Android con el protocolo ADB, qué datos recoge, los estados de un dispositivo y consejos útiles.
---
# Android

EmuLabZip es compatible con emuladores Android y dispositivos físicos con la depuración por USB o inalámbrica activada.

## Cómo se conecta

EmuLabZip habla el **protocolo de host de ADB** directamente con el servidor ADB en `127.0.0.1:5037`, el mismo servidor que
usan el comando `adb` y Android Studio. No se instala nada en el dispositivo: cada consulta es un comando de shell que
podrías ejecutar tú mismo con `adb shell`.

| Ajuste | Efecto |
|---------|--------|
| `ADB_SERVER_SOCKET=tcp:host:port` | Usa otro servidor ADB, por ejemplo uno redirigido desde una máquina remota. |
| `ANDROID_ADB_SERVER_PORT=5038` | Usa otro puerto local. |
| `EMULABZIP_ADB=/path/to/adb` | El binario de `adb` que se usa para iniciar el servidor cuando no está en ejecución. |
| `ANDROID_HOME`, `ANDROID_SDK_ROOT` | Dónde buscar el SDK. |

## Qué se recoge

| Función | Fuente |
|---------|--------|
| Lista de dispositivos | `host:devices-l`, y luego `getprop` en cada dispositivo en línea |
| Nombre | El nombre del AVD en los emuladores (`ro.boot.qemu.avd_name`), el modelo en los demás |
| Overview | `getprop`, `wm size`, `wm density`, `dumpsys battery`, `/proc/meminfo`, `uname -r`, `df` (una sola llamada por lotes) |
| Apps | `pm list packages -3` y `-s`, con los ids de proceso de `ps -A` |
| Logs | `logcat -v threadtime -T <history>` |
| Capturas de pantalla | `screencap -p` mediante `exec:` para que el PNG no se altere |

## Estados de un dispositivo

| Estado | Significado |
|-------|---------|
| Online | Arrancado y listo. |
| Booting | Conectado, pero `sys.boot_completed` aún no está establecido. EmuLabZip lo espera. |
| Unauthorized | Acepta el aviso *Allow USB debugging* en el dispositivo. |
| Offline | El servidor ADB perdió el dispositivo. Reinicia el emulador o ejecuta `adb reconnect`. |

## Consejos

- Los emuladores con imágenes de **Google Play** son builds `user`: `run-as` solo funciona con apps depurables, algo que
  importa para las herramientas de Storage de la Fase 2.
- Pueden ejecutarse varios emuladores a la vez; cambia entre ellos con el device picker.
