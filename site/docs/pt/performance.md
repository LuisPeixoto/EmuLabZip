---
source: 98d4164fe240
description: A ferramenta Performance mede CPU, memória, taxa de quadros e bateria do app e do dispositivo, com gráficos ao vivo dos últimos dois minutos.
---
# Desempenho

A ferramenta Performance amostra o app selecionado e o dispositivo enquanto ela está aberta, e desenha os últimos dois
minutos como gráficos ao vivo. Escolha o app na barra de ferramentas; sem um, só os valores do dispositivo inteiro aparecem.

## O que é medido

| Valor | Android | iOS Simulator |
|-------|---------|---------------|
| CPU do app | Tempo de usuário + sistema de `/proc/<pid>/stat`, como fração de todos os núcleos | Tempo de CPU do processo no host (`ps`), como fração de todos os núcleos do Mac |
| CPU do dispositivo | `/proc/stat`, tempo ocupado em todos os núcleos | — (o simulador compartilha a CPU do Mac) |
| Memória do app | Resident set size (`VmRSS`) | Resident set size do processo no host |
| Memória do dispositivo | `MemTotal − MemAvailable` de `/proc/meminfo` | — |
| Taxa de quadros | Quadros renderizados por segundo, de `dumpsys gfxinfo` | Fase 4, com traces do Instruments |
| Quadros com jank | Fração de quadros com jank desde a amostra anterior | Fase 4 |
| Bateria | Nível e temperatura de `dumpsys battery` | — |

Cada amostra no Android é **uma** chamada `adb shell` que executa todos os comandos de uma vez, então amostrar a cada 500 ms
continua barato. O id do processo do app é consultado a cada amostra: quando o app reinicia, os gráficos continuam com o novo processo.

## Lendo os gráficos

- As porcentagens de CPU são do **dispositivo inteiro**, como o Android Studio mostra. Em um emulador de 4 núcleos, um núcleo
  ocupado é 25%.
- A taxa de quadros só se move enquanto o app desenha. Uma tela estática não renderiza quadros, e isso é bom.
- Quadros com jank são quadros que perderam o prazo, como contados pelo `gfxinfo` da plataforma.
- Passe o mouse sobre um gráfico para ler os valores exatos. Gráficos com uma única série não têm legenda; o título nomeia o valor.

## Controles

| Controle | Efeito |
|---------|--------|
| **0.5 s · 1 s · 2 s** | Tempo entre as amostras. |
| **Pause** | Para a amostragem; os gráficos mantêm o que têm. |
| **Clear** (lixeira) | Esvazia os gráficos. |

A amostragem para quando você sai da ferramenta, então nunca roda em segundo plano.

Abaixo dos gráficos há dois cards que medem algo sob demanda: **Start time** ([guia](startup.md)) e **Energy**
([guia](energy.md)). Os dois mudam o dispositivo enquanto rodam (o app é parado e aberto; a bateria é desconectada aos olhos do
serviço de bateria) e o devolvem ao que era.

## Pela linha de comando

```bash
emulabzip perf --app com.example.app
emulabzip perf --app com.example.app --interval 500 --json | jq .appCpu
```
