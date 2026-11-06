import SwiftUI

// A small SwiftUI app to try EmuLabZip's framework detection and Layout on: a title, a counter button, a toggle
// and a list, each with an accessibility identifier.
@main
struct DemoApp: App {
    var body: some Scene {
        WindowGroup { ContentView() }
    }
}

struct ContentView: View {
    @State private var count = 0
    @State private var notifications = true

    var body: some View {
        NavigationStack {
            VStack(spacing: 16) {
                Text("SwiftUI demo")
                    .font(.largeTitle)
                    .accessibilityIdentifier("title")
                Button("Tapped \(count) times") { count += 1 }
                    .buttonStyle(.borderedProminent)
                    .accessibilityIdentifier("counter")
                Toggle("Notifications", isOn: $notifications)
                    .padding(.horizontal)
                    .accessibilityIdentifier("notifications")
                List(["Apples", "Bananas", "Cherries"], id: \.self) { fruit in
                    Text(fruit).accessibilityIdentifier("row-\(fruit)")
                }
            }
            .navigationTitle("Demo")
        }
    }
}
