import Foundation
import UIKit

// Fails on purpose, chosen by a launch argument: --crash, --abort or --exception. With --leak it leaks memory
// instead (retain cycles that nothing can free) and keeps running, so the leaks tool has a process to look at.

final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        let window = UIWindow(frame: UIScreen.main.bounds)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        self.window = window

        let arguments = ProcessInfo.processInfo.arguments
        DispatchQueue.main.async {
            if arguments.contains("--crash") {
                explode(depth: 3)
            } else if arguments.contains("--abort") {
                abort()
            } else if arguments.contains("--exception") {
                NSException(name: .invalidArgumentException, reason: "EmuLabZip test exception", userInfo: nil).raise()
            } else if arguments.contains("--leak") {
                makeLeaks(count: 5)
            }
        }
        return true
    }
}

/// Two objects that hold each other: once the function returns nothing refers to them, and nothing frees them.
final class CycleNode {
    var other: CycleNode?
    let payload = [UInt8](repeating: 7, count: 4096)
}

@inline(never)
func makeLeaks(count: Int) {
    for _ in 0..<count {
        let a = CycleNode()
        let b = CycleNode()
        a.other = b
        b.other = a
    }
}

@inline(never)
func explode(depth: Int) {
    if depth > 0 {
        explode(depth: depth - 1)
        return
    }
    fatalError("EmuLabZip test crash")
}

UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(AppDelegate.self))
