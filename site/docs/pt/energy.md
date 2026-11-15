---
source: a19d308974b9
description: Meça quanta energia um app consome durante o uso, com estimativa em mAh no Android e tempo de CPU e estado térmico no iOS Simulator.
---
# Energia

Quanta energia um app usa em um período de tempo, enquanto você o usa.

```bash
emulabzip energy com.example.app --seconds 30
emulabzip energy com.example.app --json
```

Inicie, use o app, e ele lê o resultado quando o tempo acaba, ou quando você pressiona <kbd>Ctrl+C</kbd>.

## Android

O dispositivo é desconectado aos olhos do serviço de bateria (`dumpsys battery unplug`) e as estatísticas são zeradas
(`dumpsys batterystats --reset`), para que sejam contadas no dispositivo. No fim, `dumpsys batterystats --charged
<package>` é lido para o uid do app, e a bateria é restaurada (`dumpsys battery reset`) **sempre**, também depois de um
erro ou de <kbd>Ctrl+C</kbd>.

O relatório tem:

- o **uso estimado** pelo sistema em mAh, e as partes de onde ele vem (CPU, rádio, tela, sensores…);
- o **tempo de processador** que o app usou;
- **wake locks** (por quanto tempo cada um foi mantido e quantas vezes), **alarms** e **jobs**;
- os bytes de **rede** recebidos e enviados.

Um emulador não tem bateria. O Android calcula esses números a partir de um modelo de consumo, então compare uma execução
com outra execução, não com um celular. Um app que usou muito pouco no período, ou que não estava rodando, fica sem
estimativa, e o relatório diz isso.

## iOS Simulator

O simulador não tem modelo de consumo, então não há valor em mAh. O relatório dá o **tempo de CPU** que o app usou (o que
drena primeiro uma bateria real) e o **estado térmico** do Mac (`pmset -g therm`): quando o Mac reduz a velocidade do
processador, duas execuções não são comparáveis.

## Onde encontrar

- **Desktop**: Performance → **Energy** (Start, depois Stop and read).
- **CLI**: `emulabzip energy`.
- **MCP**: `measure_energy` (no máximo 120 segundos).
