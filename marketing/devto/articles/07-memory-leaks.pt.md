---
title: Encontre vazamentos de memória e frames lentos no Android sem abrir o Android Studio
description: Tire snapshots de memória e compare, analise heap dumps atrás de Activities vazadas e grave traces do Perfetto com flame graphs, tudo pelo app ou pela CLI do EmuLabZip.
tags: android, performance, memoryleaks, debugging
series: EmuLabZip (PT)
cover: docs/images/memory.png
publish_after: 2026-12-03
pair: 07-memory-leaks
lang: pt
---

Problemas de memória demoram a aparecer porque só se mostram com o tempo: você abre uma tela cinco vezes e algo continua vivo.
O caminho de sempre é tirar o heap dump, copiar um arquivo de dezenas de megabytes, abrir num profiler e descobrir quem segura o quê.

O EmuLabZip faz o mesmo em três passos, cada um cabendo em um comando: ver para onde vai a memória do app e o que cresceu, analisar
um heap dump atrás de vazamentos e, quando o problema é tempo e não memória, gravar um trace e ler o flame graph.

![A view Memory: um snapshot comparado com o anterior](docs/images/memory.png)

## Passo 1: veja o que cresceu

Tire um **snapshot** da memória do app, use o app (abra e feche uma tela algumas vezes) e tire outro. O segundo é comparado
automaticamente com o snapshot anterior, e as categorias aparecem ordenadas pelo tamanho da variação, com uma seta e um sinal
para que a direção nunca dependa só da cor.

```bash
emulabzip mem com.example.app
emulabzip mem com.example.app --json
```

No Android, o snapshot vem do `dumpsys meminfo`: heap Java, heap nativo, código, stack, gráficos e mais, além da contagem de
objetos (views, activities, binders, WebViews). No iOS Simulator ele usa o `footprint`, o número que o iOS usa para decidir quando
encerrar um app. Uma categoria que continua crescendo a cada repetição da ação é um bom lugar para procurar um vazamento.

## Passo 2: analise um heap dump

```bash
emulabzip heapdump com.example.app --analyze
emulabzip heap analyze before-login.hprof
emulabzip heap analyze dump.hprof --json --top 20
```

O primeiro comando tira um heap dump de um app em execução e o analisa. O EmuLabZip lê o `.hprof` do Android por conta própria,
então nenhuma outra ferramenta é necessária. O app precisa ser debuggable (ou o dispositivo precisa ter uma imagem `eng` ou
`userdebug`).

O relatório tem três partes:

- **Suspected leaks**: uma `Activity` que foi destruída (ou finalizada) e continua na memória, e um `Fragment` que foi removido
  mas continua vivo, cada um com a quantidade de memória que mantém viva e o **caminho mais curto a partir de um GC root**, por exemplo
  `static field: class com.example.LeakyActivity.kept → com.example.LeakyActivity`. Leia o caminho da direita para a esquerda: a última
  linha é o objeto vazado e a anterior é quem o segura. Referências weak, soft e phantom não são seguidas, porque
  não mantêm um objeto vivo.
- **Objects that keep the most alive**, por retained size, calculado a partir da árvore de dominadores para que objetos
  compartilhados não sejam contados duas vezes.
- **Classes by size**: número de instâncias e shallow size.

Por padrão só o heap do próprio app é examinado, e `--all-heaps` inclui o zygote e a imagem do sistema. Analisar um dump de 40 MB
leva alguns segundos. Um heap dump contém os dados do app, então mantenha-o tão privado quanto o app.

## Passo 3: quando é tempo, não memória

Para jank e aberturas lentas, grave um trace:

```bash
emulabzip trace record --preset frames --duration 15 -o frames.pftrace
emulabzip trace summary frames.pftrace
emulabzip trace query frames.pftrace "select name, count(*) n from slice group by name order by n desc limit 10"
emulabzip trace cpu --app com.example.app --duration 10 --folded stacks.txt
```

O `trace record` gera um trace de sistema do Perfetto com quatro presets: CPU e escalonamento, frames e jank, memória e
personalizado. O resumo mostra a duração do trace, o quanto a CPU esteve ocupada, o número de frames e o tempo de frame nos
percentis 50, 90 e 99, o tempo de CPU por processo e por thread, e os tipos de jank por layer. O `trace query` roda PerfettoSQL
no trace. O `trace cpu` usa o `simpleperf` para obter um perfil de CPU e o mostra como um **flame graph** com as funções mais
quentes, ou exporta stacks dobradas (folded) para outras ferramentas. No iOS Simulator, o equivalente usa o `xctrace` com o Time Profiler.

![O resumo de um trace de frames do Perfetto](docs/images/traces-summary.png)

O trace processor do Perfetto só é baixado depois que você concorda, na primeira vez que usar um resumo ou uma consulta.

## Experimente

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

- Memória: [emulabzip.luispeixoto.dev/pt/docs/memory/](https://emulabzip.luispeixoto.dev/pt/docs/memory/)
- Encontrando vazamentos em heap dumps: [emulabzip.luispeixoto.dev/pt/docs/leaks/](https://emulabzip.luispeixoto.dev/pt/docs/leaks/)
- Traces e flame graphs: [emulabzip.luispeixoto.dev/pt/docs/traces/](https://emulabzip.luispeixoto.dev/pt/docs/traces/)
- Site e toda a documentação: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/pt/)
- Código-fonte (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
