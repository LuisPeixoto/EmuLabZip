---
source: 32f3425de74b
description: Teste como um app lida com uma rede lenta, com latência, com perdas ou ausente, usando presets ou valores próprios aplicados na hora pelo proxy.
---
# Condições de rede

Teste como um app lida com uma rede lenta, com latência, com perdas ou ausente. O proxy fica no meio de tudo o que o
dispositivo envia, então ele pode moldar o link.

![O menu de condições de rede em Network, com Slow 3G escolhido](images/network-conditions.png)

Em **Network**, o botão **Network** escolhe um preset ou valores seus, e a mudança vale na hora, também para
conexões que já estão abertas. Um selo mostra o que está em vigor.

| Preset | Download | Upload | Atraso | Outros |
|--------|----------|--------|-------|-------|
| Fast 3G | 1.440 kb/s | 675 kb/s | 150 ms | |
| Slow 3G | 500 kb/s | 500 kb/s | 400 ms | |
| Slow Wi-Fi | 2.000 kb/s | 1.000 kb/s | 80 ms | |
| High latency | sem limite | sem limite | 600 ms | |
| Packet loss | sem limite | sem limite | nenhum | 5% do tráfego trava |
| Offline | | | | sem conexão |

```bash
emulabzip net --conditions slow-3g
emulabzip net --down 400 --latency 100          # numbers of your own, or on top of a preset
emulabzip net --conditions offline
```

`--down` e `--up` são em kilobits por segundo, `--latency` em milissegundos e `--loss` em porcentagem.

## O que cada um faz

- **Speed** limita a velocidade dos bytes em cada sentido, em passos uniformes.
- **Delay** é somado ao que o dispositivo recebe, uma vez para cada rajada de dados que segue uma pausa. Ele representa o
  tempo de ida e volta.
- **Loss** não consegue perder pacotes: o próprio TCP do dispositivo faz a retransmissão. Ele trava uma rajada de dados por cerca de um
  timeout de retransmissão de vez em quando, com uma chance que cresce com o tamanho da rajada, que é o que um app sente.
- **Offline** recusa novas conexões e corta as abertas na próxima vez que moverem um byte, então os apps veem um
  erro de rede. Escolher **No limits** traz tudo de volta.

Vale para tudo o que passa pelo proxy: requisições decifradas e conexões repassadas. O
**HTTP/3** fala QUIC sobre UDP, que não passa por este link: com o HTTP/3 ligado, desligue-o para ver um
app ficar lento.

As condições permanecem quando o proxy é parado e iniciado de novo, e são esquecidas quando o app é fechado.
