# Android

EmuLabZip supports Android emulators and physical devices with USB or wireless debugging enabled.

## How it connects

EmuLabZip speaks the **ADB host protocol** directly to the ADB server on `127.0.0.1:5037`, the same server the `adb`
command and Android Studio use. Nothing is installed on the device: every query is a shell command you could run
yourself with `adb shell`.

| Setting | Effect |
|---------|--------|
| `ADB_SERVER_SOCKET=tcp:host:port` | Use another ADB server, for example one forwarded from a remote machine. |
| `ANDROID_ADB_SERVER_PORT=5038` | Use another local port. |
| `EMULABZIP_ADB=/path/to/adb` | The `adb` binary used to start the server when it is not running. |
| `ANDROID_HOME`, `ANDROID_SDK_ROOT` | Where to look for the SDK. |

## What is collected

| Feature | Source |
|---------|--------|
| Device list | `host:devices-l`, then `getprop` on each online device |
| Name | The AVD name for emulators (`ro.boot.qemu.avd_name`), the model otherwise |
| Overview | `getprop`, `wm size`, `wm density`, `dumpsys battery`, `/proc/meminfo`, `uname -r`, `df` (one batched call) |
| Apps | `pm list packages -3` and `-s`, with process ids from `ps -A` |
| Logs | `logcat -v threadtime -T <history>` |
| Screenshots | `screencap -p` over `exec:` so the PNG is not altered |

## Device states

| State | Meaning |
|-------|---------|
| Online | Booted and ready. |
| Booting | Connected, but `sys.boot_completed` is not set yet. EmuLabZip waits for it. |
| Unauthorized | Accept the *Allow USB debugging* prompt on the device. |
| Offline | The ADB server lost the device. Restart the emulator, or run `adb reconnect`. |

## Tips

- Emulators running **Google Play** images are `user` builds: `run-as` works only for debuggable apps, which matters
  for the Storage tools in Phase 2.
- Several emulators can run at once; switch between them with the device picker.
