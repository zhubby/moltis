import SwiftUI

@main
struct MoltisPOCApp: App {
    @StateObject private var settings: AppSettings
    @StateObject private var chatStore: ChatStore

    init() {
        let settings = AppSettings()
        _settings = StateObject(wrappedValue: settings)
        _chatStore = StateObject(wrappedValue: ChatStore(settings: settings))
    }

    var body: some Scene {
        WindowGroup("Moltis Swift POC") {
            ContentView(chatStore: chatStore, settings: settings)
        }
        .windowResizability(.contentSize)

        Settings {
            SettingsView(settings: settings)
        }
    }
}
