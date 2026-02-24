import SwiftUI

struct SettingsSectionContent: View {
    let section: SettingsSection
    @ObservedObject var settings: AppSettings

    var body: some View {
        switch section.group {
        case .general:
            generalContent
        case .security:
            securityContent
        case .integrations:
            integrationsContent
        case .systems:
            systemsContent
        }
    }
}

// MARK: - Routing

private extension SettingsSectionContent {
    @ViewBuilder
    var generalContent: some View {
        switch section {
        case .identity: identityPane
        case .environment: environmentPane
        case .memory: memoryPane
        case .notifications: notificationsPane
        case .crons: cronsPane
        case .heartbeat: heartbeatPane
        default: EmptyView()
        }
    }

    @ViewBuilder
    var securityContent: some View {
        switch section {
        case .security: securityPane
        case .tailscale: tailscalePane
        default: EmptyView()
        }
    }

    @ViewBuilder
    var integrationsContent: some View {
        switch section {
        case .channels: channelsPane
        case .hooks: hooksPane
        case .llms: llmsPane
        case .mcp: mcpPane
        case .skills: skillsPane
        case .voice: voicePane
        default: EmptyView()
        }
    }

    @ViewBuilder
    var systemsContent: some View {
        switch section {
        case .terminal: terminalPane
        case .sandboxes: sandboxesPane
        case .monitoring: monitoringPane
        case .logs: logsPane
        case .graphql: graphqlPane
        case .configuration: configurationPane
        default: EmptyView()
        }
    }
}

// MARK: - General Panes

private extension SettingsSectionContent {
    var identityPane: some View {
        VStack(alignment: .leading, spacing: 10) {
            MoltisSection {
                TextField("Display name", text: $settings.identityName)
                    .annotation("The name shown in chat headers and channel messages")
            }
            MoltisSection {
                MoltisEditorField(
                    title: "Soul Prompt",
                    text: $settings.identitySoul
                )
            }
        }
    }

    var environmentPane: some View {
        MoltisSection {
            TextField("Config directory", text: $settings.environmentConfigDir)
                .annotation("Location of moltis.toml and credentials")
            Divider()
            TextField("Data directory", text: $settings.environmentDataDir)
                .annotation("Location of databases, sessions, and logs")
        }
    }

    var memoryPane: some View {
        MoltisSection {
            Toggle("Enable memory", isOn: $settings.memoryEnabled)
                .annotation("Allow the assistant to remember context across sessions")
            Divider()
            Picker("Memory mode", selection: $settings.memoryMode) {
                ForEach(settings.memoryModes, id: \.self) { mode in
                    Text(mode.capitalized).tag(mode)
                }
            }
            .pickerStyle(.segmented)
        }
    }

    var notificationsPane: some View {
        MoltisSection {
            Toggle("Enable notifications", isOn: $settings.notificationsEnabled)
                .annotation("Show desktop notifications for incoming messages")
            Divider()
            Toggle("Play sounds", isOn: $settings.notificationsSoundEnabled)
        }
    }

    var cronsPane: some View {
        MoltisSection {
            MoltisEditorField(
                title: "Cron Definitions",
                text: $settings.cronJobs
            )
        }
    }

    var heartbeatPane: some View {
        MoltisSection {
            Toggle("Enable heartbeat", isOn: $settings.heartbeatEnabled)
                .annotation("Periodically check system health")
            Divider()
            Stepper(
                "Interval: \(settings.heartbeatIntervalMinutes) min",
                value: $settings.heartbeatIntervalMinutes,
                in: 1 ... 120
            )
        }
    }
}

// MARK: - Security Panes

private extension SettingsSectionContent {
    var securityPane: some View {
        MoltisSection {
            Toggle("Require password login", isOn: $settings.requirePassword)
                .annotation("Protect the web UI with password authentication")
            Divider()
            Toggle("Enable passkeys", isOn: $settings.passkeysEnabled)
                .annotation("Allow WebAuthn passkey login")
        }
    }

    var tailscalePane: some View {
        MoltisSection {
            Toggle("Enable Tailscale", isOn: $settings.tailscaleEnabled)
                .annotation("Expose the instance over your Tailscale network")
            Divider()
            TextField("Hostname", text: $settings.tailscaleHostname)
        }
    }
}

// MARK: - Integrations Panes

private extension SettingsSectionContent {
    var channelsPane: some View {
        MoltisSection {
            MoltisEditorField(
                title: "Channel Rules",
                text: $settings.channelRules
            )
        }
    }

    var hooksPane: some View {
        MoltisSection {
            MoltisEditorField(
                title: "Hooks Config",
                text: $settings.hooksConfig
            )
        }
    }

    var llmsPane: some View {
        MoltisSection {
            Picker("Provider", selection: $settings.llmProvider) {
                ForEach(settings.llmProviders, id: \.self) { provider in
                    Text(provider.capitalized).tag(provider)
                }
            }
            Divider()
            TextField("Model", text: $settings.llmModel)
            Divider()
            SecureField("API key", text: $settings.llmApiKey)
        }
    }

    var mcpPane: some View {
        MoltisSection {
            MoltisEditorField(
                title: "MCP Servers",
                text: $settings.mcpServers
            )
        }
    }

    var skillsPane: some View {
        MoltisSection {
            MoltisEditorField(
                title: "Skills",
                text: $settings.skills
            )
        }
    }

    var voicePane: some View {
        MoltisSection {
            Toggle("Enable voice", isOn: $settings.voiceEnabled)
                .annotation("Enable text-to-speech and speech-to-text")
            Divider()
            Picker("Voice provider", selection: $settings.voiceProvider) {
                ForEach(settings.voiceProviders, id: \.self) { provider in
                    Text(provider.capitalized).tag(provider)
                }
            }
            Divider()
            SecureField("Voice API key", text: $settings.voiceApiKey)
        }
    }
}

// MARK: - Systems Panes

private extension SettingsSectionContent {
    var terminalPane: some View {
        MoltisSection {
            Toggle("Enable terminal tool", isOn: $settings.terminalEnabled)
                .annotation("Allow the assistant to execute shell commands")
            Divider()
            TextField("Default shell", text: $settings.terminalShell)
        }
    }

    var sandboxesPane: some View {
        MoltisSection {
            Picker("Backend", selection: $settings.sandboxBackend) {
                ForEach(settings.sandboxBackends, id: \.self) { backend in
                    Text(backend.capitalized).tag(backend)
                }
            }
            Divider()
            TextField("Default image", text: $settings.sandboxImage)
        }
    }

    var monitoringPane: some View {
        MoltisSection {
            Toggle("Enable monitoring", isOn: $settings.monitoringEnabled)
            Divider()
            Toggle("Enable metrics", isOn: $settings.metricsEnabled)
                .annotation("Expose Prometheus-compatible metrics endpoint")
            Divider()
            Toggle("Enable tracing", isOn: $settings.tracingEnabled)
                .annotation("Emit OpenTelemetry traces for async operations")
        }
    }

    var logsPane: some View {
        MoltisSection {
            Picker("Log level", selection: $settings.logLevel) {
                ForEach(settings.logLevels, id: \.self) { level in
                    Text(level.uppercased()).tag(level)
                }
            }
            Divider()
            Toggle("Persist logs to disk", isOn: $settings.persistLogs)
                .annotation("Write log files to the data directory")
        }
    }

    var graphqlPane: some View {
        MoltisSection {
            Toggle("Enable GraphQL", isOn: $settings.graphqlEnabled)
                .annotation("Expose a GraphQL API for programmatic access")
            Divider()
            TextField("GraphQL path", text: $settings.graphqlPath)
        }
    }

    var configurationPane: some View {
        MoltisSection {
            MoltisEditorField(
                title: "moltis.toml",
                text: $settings.configurationToml,
                minHeight: 320
            )
        }
    }
}
