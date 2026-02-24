import SwiftUI

@main
struct MoltisApp: App {
    @StateObject private var settings: AppSettings
    @StateObject private var chatStore: ChatStore
    @StateObject private var onboardingState: OnboardingState

    init() {
        let settings = AppSettings()
        let onboardingState = OnboardingState()
        _settings = StateObject(wrappedValue: settings)
        _chatStore = StateObject(wrappedValue: ChatStore(settings: settings))
        _onboardingState = StateObject(wrappedValue: onboardingState)
    }

    var body: some Scene {
        WindowGroup("Moltis") {
            Group {
                if onboardingState.isCompleted {
                    ContentView(chatStore: chatStore, settings: settings)
                } else {
                    OnboardingView(settings: settings) {
                        onboardingState.complete()
                        chatStore.loadVersion()
                    }
                }
            }
        }
        .windowResizability(.contentSize)

        Settings {
            SettingsView(settings: settings)
        }
    }
}
