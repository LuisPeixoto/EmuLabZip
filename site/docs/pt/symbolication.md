---
source: f024f5686c62
description: Transforme stacks de builds de release em nomes legíveis com mapping.txt, source maps do React Native e dSYM, mantidos por app e versão.
---
# Stacks legíveis

Um build de release esconde os seus nomes. O R8 e o ProGuard renomeiam as classes para `a.b.c`, o React Native entrega um
único bundle JavaScript em que toda posição é `1:608617`, e um app iOS sem símbolos tem endereços puros. O EmuLabZip
lê tudo isso de volta no que você escreveu, quando tem o arquivo que o build gerou para esse fim.

| O que quebrou | O arquivo | O que você obtém |
|--------------|----------|--------------|
| Java ou Kotlin, minificado com R8 ou ProGuard | `mapping.txt` | `Cart.average` em `MainActivity.kt:39`, com as chamadas inlined devolvidas como frames próprios |
| JavaScript do React Native (Hermes ou JavaScriptCore) | o source map do bundle | `App.tsx 17:14` em vez de `1:608617` |
| Um app iOS | o seu `.dSYM` | função, arquivo e linha para cada endereço (em um Mac, com `atos`) |

<p align="center">
  <img src="images/symbolication.png" alt="O mesmo crash do Android, legível com o seu mapping.txt e como o dispositivo o escreveu" />
</p>

## Início rápido

Guarde o arquivo do app e da versão a que ele pertence, uma única vez. Todo crash daquele build passa a ser lido com ele.

```bash
emulabzip mappings add com.example.shop "1.2.0 (42)" app/build/outputs/mapping/release/mapping.txt
emulabzip crash dropbox:data_app_crash:20261023205928        # readable
emulabzip crash dropbox:data_app_crash:20261023205928 --raw  # as the device wrote it
```

No app desktop, abra **Crashes**. Um crash que poderia ser lido melhor diz o que falta e oferece **Add mapping or
source map…** e **Add dSYM…**: escolha o arquivo e o crash é lido de novo. **Readable** e **Original** alternam entre os
dois.

## A versão precisa corresponder

Um mapping só está certo para o build que o gerou, e ler outro build com ele é pior do que não ler. Por isso os arquivos
são guardados por app e versão, e um crash só é lido com os da sua própria versão. A versão é a que o crash mostra:

- **Android**: `1.2.0 (42)`, o nome da versão e o código da versão, da linha `Package:` do relatório.
- **iOS**: `1.2.0 (42)`, a versão curta e o número do build.

Quando nenhum arquivo serve, o crash diz isso e informa a versão de que precisa.

## Onde ficam os arquivos

Você pode guardá-los em qualquer lugar e indicá-los na linha de comando, um crash por vez:

```bash
emulabzip crash <id> --mapping app/build/outputs/mapping/release/mapping.txt
```

ou guardá-los de vez com `mappings add`. Eles são copiados para a pasta de dados do EmuLabZip
(`~/Library/Application Support/EmuLabZip/mappings/<app>/<version>/` no macOS), então um clean build não os leva
embora. `emulabzip mappings list` mostra o que está guardado, e `emulabzip mappings remove <app> <version>` esquece uma
versão.

O que um arquivo é se descobre a partir do próprio arquivo, então não há tipo a informar; `--kind
mapping|sourcemap|dsym` o indica quando não dá para descobrir.

## Android: mapping.txt

O R8 o grava em `app/build/outputs/mapping/<variant>/mapping.txt` quando `isMinifyEnabled` está ativado. Guarde-o para
toda versão que você publicar: é o único caminho de volta dos nomes em um crash.

- Os frames são devolvidos com classe, método, arquivo e linha.
- Quando o R8 fez o inline de um método em quem o chama, o único frame embaralhado vira a cadeia inteira, com a chamada
  mais interna primeiro, como seria sem o R8.
- Os nomes de classe no título da exceção também são restaurados.
- Mantenha `-keepattributes SourceFile,LineNumberTable` nas suas regras, ou não haverá linhas para devolver.

## React Native: source maps

O JavaScript de um crash do React Native chega no relatório como um texto de exceção, não como frames. O EmuLabZip o lê
em uma thread **JavaScript**, com as threads de Java e nativas depois dela. As posições são procuradas no source map do
bundle:

- Os builds de release do **Hermes** gravam um mapa composto,
  `android/app/build/generated/sourcemaps/react/release/index.android.bundle.map`. Use esse; os mapas `packager` e
  `compiler` ao lado dele são as duas metades de que ele é feito.
- Os bundles do **JavaScriptCore** e do **Metro** usam o mapa que `react-native bundle --sourcemap-output` grava.

## iOS: dSYM

O Xcode grava um `.dSYM` ao lado de cada archive. O EmuLabZip confere se o UUID dele é o da imagem que quebrou e depois
pergunta ao `atos` por todos os endereços daquela imagem de uma vez. Isso exige um Mac, porque o `atos` faz parte das
ferramentas do Xcode; no Linux e no Windows, um crash com endereços puros diz isso, e um crash que já tem nomes é
mostrado como está. O Simulator muitas vezes simboliza o próprio relatório de crash quando encontra o dSYM, e aí não
sobra nada a fazer.

## Uma stack de outro lugar

Uma stack que você copiou de um log, de um relatório de bug ou de uma execução de CI pode ser lida do mesmo jeito:

```bash
emulabzip symbolicate stack.txt --mapping mapping.txt
adb logcat -d | emulabzip symbolicate - --mapping mapping.txt
```

Os frames Java são lidos com um `mapping.txt` e os frames JavaScript com um source map. Um `.dSYM` precisa dos endereços
e da imagem de um relatório de crash, que uma stack colada não tem.

## Para assistentes

`get_crash` lê o crash com o que está guardado para o seu app e versão, e diz quais arquivos usou e o que falta
(`symbolication`). `original: true` mantém os nomes como o dispositivo os escreveu. Veja [MCP](mcp.md).
