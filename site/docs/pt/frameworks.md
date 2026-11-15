---
source: dcdd776b1e52
description: "Veja por dentro de apps Flutter, React Native, Jetpack Compose e SwiftUI: detecção do framework, árvore de widgets e componentes, frames, memória e console."
---
# Por dentro dos frameworks

O Flutter e o React Native desenham a própria tela, e o Jetpack Compose e o SwiftUI desenham a deles de modo diferente
das views da plataforma. A árvore de views usual diz pouco sobre eles, então o EmuLabZip descobre qual framework um app
usa e olha por dentro com as ferramentas do próprio framework.

```bash
emulabzip frameworks com.example.app
```

Ele lê os arquivos do app, esteja ele em execução ou não:

| Framework | O que o entrega |
|-----------|--------------------|
| Flutter | `libflutter.so` no APK, ou `Flutter.framework` no bundle do app |
| React Native | `libreactnative.so`, `libreactnativejni.so` ou `index.android.bundle`; `main.jsbundle` ou `React.framework` no iOS |
| Hermes | `libhermes.so` ou `libhermesvm.so`; `hermes.framework` |
| Jetpack Compose | os arquivos `META-INF/androidx.compose.*.version` que as bibliotecas gravam no APK (a versão é lida deles) |
| SwiftUI | `SwiftUI.framework` entre as bibliotecas com que o executável é ligado |

No app desktop, **Frameworks** mostra o mesmo, e os painéis do Flutter e do React Native logo abaixo.

## Flutter

![A árvore de widgets de um app Flutter na view Frameworks, com o arquivo e a linha de cada widget que o app criou](images/frameworks-flutter.png)

Os builds de debug e profile têm um **Dart VM service**, que o app anuncia no log:
`The Dart VM service is listening on http://127.0.0.1:41233/token=/`. O EmuLabZip encontra essa linha, alcança a porta
pelo dispositivo (`adb` no Android, a própria rede do Mac no simulador, nada é encaminhado nem deixado para trás) e fala
JSON-RPC por um WebSocket.

```bash
emulabzip flutter tree com.example.app
emulabzip flutter frames com.example.app --seconds 10     # use the app meanwhile
emulabzip flutter memory com.example.app
```

- **Widgets**: a árvore que o inspetor do Flutter mostra. `●` marca os widgets criados pelo seu próprio código, com o
  arquivo e a linha; `○` marca os do framework e dos pacotes. Os nomes de arquivo exigem que o app saiba onde seus
  widgets foram criados. Os apps iniciados com `flutter run` sabem. Para `flutter build`, adicione
  `--extra-front-end-options=--track-widget-creation` (o `tools/flutter-app/install.sh` faz isso). Sem isso, a árvore
  não tem nomes de arquivo e informa isso.
- **Frames**: o tempo de build (a thread de UI) e o tempo de raster (a thread de desenho) de cada frame que o app
  desenha, com os frames com jank contados: um frame tem jank quando qualquer um dos dois passa de 16,7 ms. **Nada é
  desenhado enquanto o app está ocioso**, então o número de frames depende do que você faz enquanto ele grava. Um
  emulador sem placa gráfica faz o raster por software, então seus tempos de raster dizem pouco sobre um celular.
- **Memory**: o heap do Dart em uso e o seu tamanho, a memória fora do heap que os objetos do Dart mantêm viva (imagens,
  buffers) e as classes que mais usam.

Se o endereço não estiver mais no log (o log de um app movimentado rola), inicie o app de novo ou informe-o:
`--uri http://127.0.0.1:41233/token=/`. Um build de release não tem VM service.

## React Native

![A árvore de componentes React de um app React Native na view Frameworks, com props](images/frameworks-react-native.png)

Os apps React Native em desenvolvimento conversam com o **Metro**, o servidor de desenvolvimento, que lista o runtime
JavaScript de cada app conectado e faz proxy do seu protocolo Chrome DevTools. O EmuLabZip se conecta ao Metro no seu
computador (porta 8081, ou `--metro <port>`), escolhe o runtime que pertence ao app e lê:

```bash
emulabzip rn tree com.example.app           # the React components, with props
emulabzip rn heap com.example.app           # Hermes' heap in use
emulabzip rn console com.example.app --seconds 20
```

- **Components**: a árvore do React, lida do hook do React DevTools que um build de desenvolvimento instala. `●` marca
  os componentes, `○` as views nativas em que eles se transformam. As props são abreviadas: `title="Hello"`,
  `onPress=ƒ`.
- **Hermes heap** e a versão do Hermes.
- **Console**: o que o app escreve com `console.log` e afins enquanto você escuta.

A aba **State** (e `emulabzip rn state`) lê Redux, React Query, o contexto do React e os seus próprios stores
registrados: veja [Estado do React Native](react-native-state.md).

**O Metro deixa apenas um depurador se conectar a um app por vez.** Ler pelo EmuLabZip pode fechar o React Native
DevTools ou o inspetor do Chrome, e vice-versa. Os builds de release não têm Metro nem hook.

No Android, o app alcança o Metro por `adb reverse tcp:8081 tcp:8081`, que o EmuLabZip não mexe.

## Jetpack Compose e SwiftUI

![Uma tela de Jetpack Compose em Layout, com suas views marcadas como Compose e o inspetor mostrando o nome do framework](images/layout-compose.png)

Em **Layout**, as views que um framework desenha são marcadas, e o inspetor mostra qual. Tudo o que está sob uma
`ComposeView` é Compose; as test tags (`Modifier.testTag`) aparecem como ids quando o app ativa
`testTagsAsResourceId`. Sob uma `FlutterView` ou uma view raiz do React Native, as marcas dizem `flutter` e
`react-native`.

No iOS, a árvore de acessibilidade não tem uma classe que diga o que desenhou uma view, então, com
`layout --app <bundle id>` (e selecionando um app no app desktop), a tela inteira é marcada com o framework do app:
Flutter, React Native, Compose e depois SwiftUI, nessa ordem de preferência. Os tipos de view próprios do SwiftUI
(`VStack`, `Button`) não estão disponíveis pela acessibilidade: você vê os papéis de acessibilidade e os identificadores
que você deu.

**As contagens de recomposição não estão disponíveis.** O Layout Inspector do Android Studio as obtém de um agente
carregado no app depurado, que usa interfaces que não são públicas. O EmuLabZip usa apenas interfaces públicas, então
não as mostra.

## Experimentando

O `tools/` tem um app de demonstração para cada um: `compose-app`, `flutter-app`, `rn-app` e `swiftui-app`, cada um com
um README.
