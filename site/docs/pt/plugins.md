---
source: 7a600d6f8eb4
description: "Um plugin é um coletor seu: uma pasta com plugin.json e um programa que imprime JSON, e o EmuLabZip desenha métricas, tabelas, texto e linhas de log."
---
# Plugins

Um plugin é um coletor seu. É uma pasta com um `plugin.json` e um programa. O EmuLabZip inicia o programa para um
dispositivo, o programa imprime um objeto JSON por linha na saída padrão, e o EmuLabZip os desenha: métricas como gráficos
ou tiles, tabelas, texto e linhas de log.

Não há código de plugin dentro do app. Um plugin não pode mudar a interface nem quebrá-la, e só pode fazer o que um programa
iniciado por você mesmo poderia fazer.

<p align="center">
  <img src="images/plugins.png" alt="Um plugin de streaming traçando o gráfico da temperatura da bateria" />
</p>

## Instalar, ativar, executar

```bash
emulabzip plugin new my-plugin                       # a template in the plugins folder
emulabzip plugin install examples/plugins/top-processes   # copy a plugin folder in
emulabzip plugin list
emulabzip plugin enable my-plugin
emulabzip plugin run my-plugin --app com.example.app
```

No app, abra **Plugins**. Os plugins ficam na pasta `plugins` dentro da pasta de dados do EmuLabZip (**Open folder**).

**Um plugin é um programa que roda como você.** Por isso ele nunca roda até você ativá-lo, e ativar mostra o comando exato e
de onde ele roda. A aprovação está ligada ao conteúdo do `plugin.json`: mude o arquivo e o plugin é desativado de novo. O app
e o servidor MCP só executam plugins ativados. Na linha de comando, `emulabzip plugin run <name>` é um pedido seu para
executá-lo. Leia um plugin antes de ativá-lo.

## `plugin.json`

```json
{
  "protocol": 1,
  "name": "battery-temperature",
  "version": "1.0.0",
  "description": "Plots the battery temperature of an Android device.",
  "platforms": ["android"],
  "command": ["./run.sh"],
  "mode": "stream",
  "timeoutS": 60
}
```

| Campo | Significado |
|-------|---------|
| `protocol` | Deve ser `1`. |
| `name` | Letras, dígitos, `-` e `_`. Deve ser o nome da pasta. |
| `version`, `description` | Mostrados na lista. |
| `platforms` | `android`, `ios` ou ambos. Vazio significa ambos. |
| `command` | O programa e seus argumentos, nunca executados por um shell. O programa é procurado primeiro na pasta do plugin, depois no PATH. |
| `mode` | `once`: o programa imprime seus resultados e termina. `stream`: ele continua rodando e imprime conforme avança, até ser parado. |
| `timeoutS` | Para `once`: segundos antes de o programa ser parado (padrão 60, no máximo 600). |

## O que o programa recebe

O diretório de trabalho é a pasta do plugin, e o ambiente tem:

| Variável | Valor |
|----------|-------|
| `EMULABZIP_DEVICE` | O serial do adb ou o UDID do simulador |
| `EMULABZIP_PLATFORM` | `android` ou `ios` |
| `EMULABZIP_APP` | O app escolhido (nome do pacote ou bundle id), vazio quando não há |
| `EMULABZIP_ADB` | O caminho do `adb` (a cópia do SDK, ou `adb`) |
| `EMULABZIP_PLUGIN_DIR` | A pasta do plugin |
| `EMULABZIP_PROTOCOL` | `1` |

## O que o programa imprime

Um objeto JSON por linha, com um `type`:

```json
{"type":"metric","name":"battery temperature","value":25.0,"unit":"°C"}
{"type":"table","title":"Top processes","columns":["PID","MB","Name"],"rows":[[552,217.9,"system_server"]]}
{"type":"text","title":"Picked app","body":"com.example uses 120 MB"}
{"type":"log","level":"warn","message":"adb was slow"}
{"type":"error","message":"The device did not answer"}
```

- `metric`: um número. No modo `stream` cada valor é um ponto em um gráfico (os últimos 120 são mantidos); no modo `once` é um
  tile.
- `table`: as células são texto, números, booleanos ou null. Uma linha pode ser mais curta que as colunas, nunca mais longa, e
  as células não podem conter listas nem objetos.
- `text`, `log`: mostrados como estão.
- `error`: mostrado como um erro. Um programa que sai com um código de erro e não diz nada recebe um erro feito a partir do seu
  código de saída e do fim da sua saída de erro.

Linhas que não são JSON, e eventos de um tipo que esta versão não conhece, são ignorados, então um plugin pode imprimir
progresso para pessoas e usar eventos mais novos com segurança. Faça flush depois de cada linha quando o plugin fizer
streaming.

## Limites

Uma linha pode ter até 256 KB (uma mais longa é ignorada), e uma execução pode imprimir até 8 MB no total, e depois disso o
plugin é parado. Um plugin `once` que passa do seu timeout é parado. Parar uma execução, fechar o app ou desativar o plugin
encerra o programa.

## Exemplos

`examples/plugins` tem dois: `battery-temperature` (um script `sh` que faz streaming de uma métrica) e `top-processes` (um
script Python que imprime uma tabela). `emulabzip plugin new` grava um template em Python que imprime uma métrica e algum
texto.
