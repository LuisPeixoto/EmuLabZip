# Energy

How much energy an app uses over a stretch of time, while you use it.

```bash
emulabzip energy com.example.app --seconds 30
emulabzip energy com.example.app --json
```

Start it, use the app, and it reads the result when the time is up, or when you press <kbd>Ctrl+C</kbd>.

## Android

The device is unplugged in the battery service's eyes (`dumpsys battery unplug`) and the statistics are reset
(`dumpsys batterystats --reset`), so that they are counted on the device. At the end `dumpsys batterystats --charged
<package>` is read for the app's uid, and the battery is put back (`dumpsys battery reset`) **always**, also after an
error or <kbd>Ctrl+C</kbd>.

The report has:

- the system's **estimated use** in mAh, and the parts it comes from (CPU, radio, screen, sensors…);
- the **processor time** the app used;
- **wake locks** (how long each was held and how many times), **alarms** and **jobs**;
- the **network** bytes received and sent.

An emulator has no battery. Android computes these numbers from a power model, so compare one run with another run, not
with a phone. An app that used too little in the time, or that was not running, gets no estimate, and the report says so.

## iOS Simulator

The simulator has no power model, so there is no figure in mAh. The report gives the **CPU time** the app used (what
drains a real battery first) and the Mac's **thermal state** (`pmset -g therm`): when the Mac slows its processor down,
two runs are not comparable.

## Where to find it

- **Desktop**: Performance → **Energy** (Start, then Stop and read).
- **CLI**: `emulabzip energy`.
- **MCP**: `measure_energy` (at most 120 seconds).
