---
source: 32f3425de74b
description: Prueba cómo se comporta una app con una red lenta, con latencia, con pérdidas o sin conexión, usando presets o tus propios números en el proxy.
---
# Condiciones de red

Prueba cómo se las arregla una app con una red lenta, con latencia, con pérdidas o inexistente. El proxy está en medio de todo lo
que envía el dispositivo, así que puede dar forma al enlace.

![El menú de condiciones de red en Network, con Slow 3G elegido](images/network-conditions.png)

En **Network**, el botón **Network** elige un preset o números propios, y el cambio se aplica de inmediato, también a las
conexiones que ya están abiertas. Una insignia muestra lo que está en vigor.

| Preset | Descarga | Subida | Retraso | Otros |
|--------|----------|--------|---------|-------|
| Fast 3G | 1.440 kb/s | 675 kb/s | 150 ms | |
| Slow 3G | 500 kb/s | 500 kb/s | 400 ms | |
| Slow Wi-Fi | 2.000 kb/s | 1.000 kb/s | 80 ms | |
| High latency | sin límite | sin límite | 600 ms | |
| Packet loss | sin límite | sin límite | ninguno | el 5 % del tráfico se detiene |
| Offline | | | | sin conexión |

```bash
emulabzip net --conditions slow-3g
emulabzip net --down 400 --latency 100          # numbers of your own, or on top of a preset
emulabzip net --conditions offline
```

`--down` y `--up` están en kilobits por segundo, `--latency` en milisegundos y `--loss` en porcentaje.

## Qué hace cada uno

- **Speed** limita la velocidad a la que van los bytes en cada sentido, en pasos uniformes.
- **Delay** se añade a lo que recibe el dispositivo, una vez por cada ráfaga de datos que sigue a una pausa. Representa el
  tiempo de ida y vuelta.
- **Loss** no puede perder paquetes: el propio TCP del dispositivo hace la retransmisión. Detiene de vez en cuando una ráfaga de datos durante alrededor de un
  timeout de retransmisión, con una probabilidad que crece con el tamaño de la ráfaga, que es lo que siente una app.
- **Offline** rechaza las conexiones nuevas y corta las abiertas la próxima vez que muevan un byte, así que las apps ven un
  error de red. Elegir **No limits** lo devuelve todo.

Se aplica a todo lo que pasa por el proxy: las peticiones que se descifran y las conexiones que pasan sin cambios. **HTTP/3**
habla QUIC sobre UDP, que no pasa por este enlace: con HTTP/3 activado, desactívalo para ver una app ralentizada.

Las condiciones se mantienen cuando el proxy se detiene y se vuelve a iniciar, y se olvidan cuando se cierra la app.
