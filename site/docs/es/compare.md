---
source: dabee4094ffd
description: "Compara dos dispositivos lado a lado mientras usas tu app en ambos: pantallas, gráficos de CPU y memoria, y las diferencias entre sus características."
---
# Comparar dos dispositivos

**Compare** pone dos dispositivos lado a lado mientras usas tu app en ambos: un Android antiguo junto a uno nuevo, un Pixel junto
a un iPhone, un teléfono junto a una tableta.

Inicia un segundo emulador o simulador y aparecerá en el selector **against**. La vista tiene entonces:

- **Both screens**, actualizadas cada pocos segundos.
- **Gráficos de CPU y memoria** de ambos a la vez, para la app elegida en la barra de herramientas cuando se ejecuta en los dos (de todo el
  dispositivo en caso contrario), para que veas cuál trabaja más.
- **Differences**: cada dato que informan los dos dispositivos (SO, build, hardware, pantalla, batería, idioma) alineado, con lo que
  difiere resaltado. Marca **Show what matches** para ver la lista completa.

Desde un terminal:

```bash
emulabzip compare emulator-5554 "iPhone 17"          # only what differs
emulabzip compare emulator-5554 emulator-5556 --all  # everything
emulabzip compare emulator-5554 emulator-5556 --json
```

Los datos que un dispositivo no tiene se muestran como `—`. Se permite comparar un dispositivo Android con uno iOS, aunque la mayoría de las filas
diferirán porque las plataformas nombran las cosas de forma distinta.
