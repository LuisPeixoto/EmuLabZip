---
source: 854d8470ae63
description: Como o EmuLabZip se conecta a emuladores e dispositivos Android via ADB, o que ele coleta de cada um, os estados possíveis de um dispositivo e dicas úteis.
---
# Android

O EmuLabZip oferece suporte a emuladores Android e a dispositivos físicos com depuração por USB ou sem fio ativada.

## Como ele se conecta

O EmuLabZip fala o **protocolo de host do ADB** diretamente com o servidor ADB em `127.0.0.1:5037`, o mesmo servidor que
o comando `adb` e o Android Studio usam. Nada é instalado no dispositivo: cada consulta é um comando de shell que você
mesmo poderia executar com `adb shell`.

| Configuração | Efeito |
|---------|--------|
| `ADB_SERVER_SOCKET=tcp:host:port` | Usa outro servidor ADB, por exemplo um encaminhado de uma máquina remota. |
| `ANDROID_ADB_SERVER_PORT=5038` | Usa outra porta local. |
| `EMULABZIP_ADB=/path/to/adb` | O binário `adb` usado para iniciar o servidor quando ele não está em execução. |
| `ANDROID_HOME`, `ANDROID_SDK_ROOT` | Onde procurar o SDK. |

## O que é coletado

| Recurso | Origem |
|---------|--------|
| Lista de dispositivos | `host:devices-l` e depois `getprop` em cada dispositivo online |
| Nome | O nome do AVD nos emuladores (`ro.boot.qemu.avd_name`), o modelo nos demais |
| Overview | `getprop`, `wm size`, `wm density`, `dumpsys battery`, `/proc/meminfo`, `uname -r`, `df` (uma chamada em lote) |
| Apps | `pm list packages -3` e `-s`, com os ids de processo de `ps -A` |
| Logs | `logcat -v threadtime -T <history>` |
| Screenshots | `screencap -p` sobre `exec:` para que o PNG não seja alterado |

## Estados do dispositivo

| Estado | Significado |
|-------|---------|
| Online | Inicializado e pronto. |
| Booting | Conectado, mas `sys.boot_completed` ainda não foi definido. O EmuLabZip espera por ele. |
| Unauthorized | Aceite o aviso *Allow USB debugging* no dispositivo. |
| Offline | O servidor ADB perdeu o dispositivo. Reinicie o emulador ou execute `adb reconnect`. |

## Dicas

- Emuladores com imagens **Google Play** são builds `user`: o `run-as` só funciona para apps depuráveis, o que importa
  para as ferramentas de Storage na Phase 2.
- Vários emuladores podem rodar ao mesmo tempo; alterne entre eles com o device picker.
