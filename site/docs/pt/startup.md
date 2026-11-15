---
source: fb1852db95e8
description: Meça quanto tempo um app leva para abrir, de forma repetível, com início frio, morno ou quente e a mediana de várias execuções no Android e no iOS.
---
# Tempo de abertura do app

Quanto tempo um app leva para abrir, medido sempre do mesmo jeito e repetido algumas vezes para que a mediana signifique algo.

```bash
emulabzip startup com.example.app                 # cold, 5 runs
emulabzip startup com.example.app --kind hot --runs 10
emulabzip startup com.example.app --json
```

| Tipo | Android | iOS Simulator |
|------|---------|---------------|
| **cold** | O processo é parado antes (`am force-stop`), então o sistema o inicia do zero | O app é encerrado antes |
| **warm** | A tela é fechada com o botão voltar e o processo é mantido | Indisponível: o simulador não tem botão voltar, então não há início morno separado |
| **hot** | O app é enviado para o segundo plano com o botão home | O app Settings é colocado na frente |

## O que é medido

- O **Android** usa `am start -W`, que informa `TotalTime` (o tempo até o primeiro quadro ser desenhado), `WaitTime` e o
  `LaunchState`. A activity de launcher é encontrada com `cmd package resolve-activity`.
- O **iOS Simulator** lê o log unificado. O SpringBoard registra quando é pedido a ele que abra um app e de novo quando a
  scene do app fica ativa. O tempo entre os dois é o que uma pessoa espera, animação incluída. Compare execuções entre si,
  não com um celular.

O resultado lista cada execução e a mediana, a mais rápida, a mais lenta e o percentil 90. A mediana é o número a acompanhar;
uma execução lenta costuma ser ruído.

## Onde encontrar

- **Desktop**: Performance → **Start time** (tipo, número de execuções, uma barra por execução).
- **CLI**: `emulabzip startup`.
- **MCP**: `measure_startup`.
- **CI**: com uma tabela `[startup]` em [`emulabzip.toml`](budgets.md), `emulabzip ci --app …` mede antes do comando
  e limita a mediana a `start_time_ms`.

O app é parado e aberto no dispositivo enquanto isso roda, então não execute em um dispositivo onde isso custe trabalho a você.
