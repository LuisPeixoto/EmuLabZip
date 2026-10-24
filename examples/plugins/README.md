# Example plugins

Two small plugins for EmuLabZip. Install them with:

```bash
emulabzip plugin install examples/plugins/battery-temperature
emulabzip plugin install examples/plugins/top-processes
```

| Plugin | Mode | Shows |
|--------|------|-------|
| `battery-temperature` | `stream` | The battery temperature of an Android device, as a chart that updates every two seconds. Plain `sh`. |
| `top-processes` | `once` | The processes using the most memory, as a table, and the memory of the picked app. Python 3. |

Then enable them (`emulabzip plugin enable <name>`, or the switch in the app's Plugins view) and run one:

```bash
emulabzip plugin run top-processes --app com.android.systemui
```

Read `run.sh` and `run.py` before you enable a plugin. A plugin is a program that runs as you. See
[Plugins](../../docs/plugins.md) for the protocol and how to write your own, or start from a template with
`emulabzip plugin new my-plugin`.
