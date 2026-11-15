---
source: 6a33aedfa838
description: O EmuLabZip lê um .hprof do Android e um .memgraph do iOS e lista Activities vazadas, caminhos até a raiz do GC e objetos que mais retêm memória.
---
# Encontrando vazamentos em heap dumps

O EmuLabZip lê um `.hprof` do Android por conta própria e lista o que mantém a memória viva, sem precisar de nenhuma outra ferramenta.

```bash
emulabzip heapdump com.example.app --analyze         # take a dump and analyze it
emulabzip heap analyze before-login.hprof            # a dump you already have
emulabzip heap analyze dump.hprof --json --top 20
```

O app precisa ser **depurável** (ou o dispositivo uma imagem `eng`/`userdebug`) para gerar o dump.

## O que ele informa

**Vazamentos suspeitos**, os casos que são vazamentos sem dúvida, com o caminho que mantém cada um vivo:

- uma `Activity` que foi **destruída** (`mDestroyed`), ou foi **finalizada** (`mFinished`, informada apenas como *possível*,
  porque o sistema pode ainda não a ter destruído) e continua na memória;
- um `Fragment` (`androidx.fragment.app` ou `android.app`) sem `mFragmentManager`, o que significa que ele foi removido.

Para cada um: a classe e o endereço, **quanta memória ele mantém viva** e o caminho mais curto a partir de uma raiz do GC,
por exemplo:

```text
static field: class com.example.LeakyActivity.kept → com.example.LeakyActivity
```

Leia o caminho da direita para a esquerda: a última linha é o objeto vazado, e a entrada antes dela é o que o segura. As
referências weak, soft e phantom (`Reference.referent`) não são seguidas, porque não mantêm um objeto vivo.

**Objetos que mais mantêm memória viva**, por retained size. O retained size é o que seria liberado se o objeto fosse
coletado; ele é calculado a partir da árvore de dominadores, então objetos compartilhados não são contados duas vezes.

**Classes por tamanho**: número de instâncias e shallow size.

Por padrão, só o heap do próprio app é examinado; `--all-heaps` inclui o zygote e a imagem do sistema.

## Experimente

`tools/leak-app` é um app pequeno e depurável que vaza uma Activity de propósito (veja o README dele):

```bash
tools/leak-app/install.sh
adb shell am start -n dev.emulabzip.leaky/.MainActivity --es mode leak
emulabzip heapdump dev.emulabzip.leaky --analyze
```

## iOS Simulator

Um `.memgraph` é lido com a ferramenta `leaks` do Mac, então precisa de um Mac. O relatório lista os **ciclos de retenção e
objetos vazados** por tipo, com seus tamanhos (não há retained sizes). `tools/crash-app-ios --leak` cria um ciclo de
retenção para você testar.

## Onde encontrar

- **Desktop**: Memory → **Heap dump…** e depois **Find leaks**, ou **Open dump…** para um arquivo em disco. Três abas:
  Suspected leaks, Dominators e Classes.
- **CLI**: `emulabzip heap analyze`, `emulabzip heapdump --analyze`.
- **MCP**: `analyze_heap` (Android).

Analisar um dump de 40 MB leva alguns segundos. Um heap dump contém os dados do app (veja [Segurança](../SECURITY.md)), então
mantenha-o tão privado quanto o app.
