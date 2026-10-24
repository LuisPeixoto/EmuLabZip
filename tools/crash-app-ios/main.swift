import Foundation
import UIKit

// Fails on purpose, chosen by a launch argument: --crash, --abort or --exception.

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
            }
        }
        return true
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
