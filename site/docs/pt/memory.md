---
source: e2d7a7419077
description: A ferramenta Memory mostra para onde vai a memória do app, compara dois momentos para ver o que cresceu e gera heap dumps no Android e no iOS.
---
# Memória

A ferramenta Memory mostra para onde vai a memória do app selecionado e deixa você comparar dois momentos para ver o que cresceu.

## Snapshots

Clique em **Take snapshot** para registrar um detalhamento:

| | Android | iOS Simulator |
|---|---|---|
| Fonte | `dumpsys meminfo <package>` | `footprint --json <pid>` no Mac |
| Total | PSS (proportional set size) | Footprint, o número que o iOS usa para decidir quando encerrar um app |
| Categorias | Java heap, native heap, código, stack, gráficos, outros privados, sistema | Regiões de VM como `MALLOC_SMALL`, `__DATA`, `CoreAnimation` |
| Detalhes | RSS total, swap e contagem de objetos (views, activities, binders, WebViews…) | Memória limpa e recuperável, número de regiões |

Tire um snapshot, use o app (abra e feche uma tela algumas vezes) e tire outro. O segundo snapshot é comparado
automaticamente com o anterior; escolha qualquer outra baseline no menu **Compared with**. As categorias são listadas pelo
tamanho da mudança, com uma seta e um sinal para que a direção nunca dependa só da cor.

Uma categoria que continua crescendo ao longo de ações repetidas é um bom lugar para procurar um vazamento.

## Heap dumps

| | Android | iOS Simulator |
|---|---|---|
| Botão | **Heap dump…** | **Memory graph…** |
| Arquivo | `.hprof`, escrito com `am dumpheap` | `.memgraph`, escrito com `leaks --outputGraph` |
| Abra com | Android Studio, Eclipse MAT ou [Perfetto](https://ui.perfetto.dev) | Xcode (File › Open), `leaks`, `heap`, `vmmap` |
| Requisito | Um app **depurável** (ou uma imagem `eng`/`userdebug`) | Nenhum |

No Android, o dump é escrito em `/data/local/tmp`, copiado para o seu computador e removido do dispositivo.

O EmuLabZip também pode ler o dump por conta própria: **Find leaks** (ou **Open dump…** para um arquivo em disco) lista as
Activities destruídas e os Fragments removidos que ainda estão vivos, o caminho que mantém cada um e os objetos que mantêm
mais memória viva. Veja [Encontrando vazamentos](leaks.md).

## Pela linha de comando

```bash
emulabzip mem com.example.app
emulabzip mem com.example.app --json
emulabzip heapdump com.example.app -o before-login.hprof
emulabzip heapdump com.example.app --analyze
emulabzip heap analyze before-login.hprof
```
