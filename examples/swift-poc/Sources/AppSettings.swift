import Combine
import Foundation

final class AppSettings: ObservableObject {
    @Published var identityName = "Moltis"
    @Published var identitySoul = ""

    @Published var environmentConfigDir = ""
    @Published var environmentDataDir = ""

    @Published var memoryEnabled = true
    @Published var memoryMode = "workspace"

    @Published var notificationsEnabled = true
    @Published var notificationsSoundEnabled = false

    @Published var cronJobs = ""
    @Published var heartbeatEnabled = true
    @Published var heartbeatIntervalMinutes = 5

    @Published var requirePassword = true
    @Published var passkeysEnabled = true
    @Published var tailscaleEnabled = false
    @Published var tailscaleHostname = ""

    @Published var channelRules = ""
    @Published var hooksConfig = ""

    @Published var llmProvider = "openai"
    @Published var llmModel = "gpt-4.1"
    @Published var llmApiKey = ""

    @Published var mcpServers = ""
    @Published var skills = ""

    @Published var voiceEnabled = false
    @Published var voiceProvider = "none"
    @Published var voiceApiKey = ""

    @Published var terminalEnabled = false
    @Published var terminalShell = "/bin/zsh"

    @Published var sandboxBackend = "auto"
    @Published var sandboxImage = "moltis/sandbox:latest"

    @Published var monitoringEnabled = true
    @Published var metricsEnabled = true
    @Published var tracingEnabled = true

    @Published var logLevel = "info"
    @Published var persistLogs = true

    @Published var graphqlEnabled = false
    @Published var graphqlPath = "/graphql"

    @Published var configurationToml = "[server]\nport = \"invalid\""

    let memoryModes = ["workspace", "global", "off"]
    let voiceProviders = ["none", "openai", "elevenlabs"]
    let sandboxBackends = ["auto", "docker", "apple-container"]
    let logLevels = ["trace", "debug", "info", "warn", "error"]
}
