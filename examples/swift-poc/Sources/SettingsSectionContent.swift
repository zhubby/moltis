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

private extension SettingsSectionContent {
    @ViewBuilder
    private var generalContent: some View {
        switch section {
        case .identity:
            identityPane
        case .environment:
            environmentPane
        case .memory:
            memoryPane
        case .notifications:
            notificationsPane
        case .crons:
            cronsPane
        case .heartbeat:
            heartbeatPane
        default:
            EmptyView()
        }
    }

    @ViewBuilder
    private var securityContent: some View {
        switch section {
        case .security:
            securityPane
        case .tailscale:
            tailscalePane
        default:
            EmptyView()
        }
    }

    @ViewBuilder
    private var integrationsContent: some View {
        switch section {
        case .channels:
            channelsPane
        case .hooks:
            hooksPane
        case .llms:
            llmsPane
        case .mcp:
            mcpPane
        case .skills:
            skillsPane
        case .voice:
            voicePane
        default:
            EmptyView()
        }
    }

    @ViewBuilder
    private var systemsContent: some View {
        switch section {
        case .terminal:
            terminalPane
        case .sandboxes:
            sandboxesPane
        case .monitoring:
            monitoringPane
        case .logs:
            logsPane
        case .graphql:
            graphqlPane
        case .configuration:
            configurationPane
        default:
            EmptyView()
        }
    }

    private var identityPane: some View {
        settingsPanel(
            title: "Identity",
            subtitle: "Assistant identity and default soul behavior."
        ) {
            TextField("Display name", text: $settings.identityName)
            editor(
                title: "Soul Prompt",
                text: $settings.identitySoul,
                minHeight: 180
            )
        }
    }

    private var environmentPane: some View {
        settingsPanel(
            title: "Environment",
            subtitle: "Core config and data directory locations."
        ) {
            TextField("Config directory", text: $settings.environmentConfigDir)
            TextField("Data directory", text: $settings.environmentDataDir)
        }
    }

    private var memoryPane: some View {
        settingsPanel(
            title: "Memory",
            subtitle: "Memory behavior and scope."
        ) {
            Toggle("Enable memory", isOn: $settings.memoryEnabled)
            Picker("Memory mode", selection: $settings.memoryMode) {
                ForEach(settings.memoryModes, id: \.self) { mode in
                    Text(mode.capitalized).tag(mode)
                }
            }
            .pickerStyle(.segmented)
        }
    }

    private var notificationsPane: some View {
        settingsPanel(
            title: "Notifications",
            subtitle: "Desktop notifications and sound behavior."
        ) {
            Toggle("Enable notifications", isOn: $settings.notificationsEnabled)
            Toggle("Play sounds", isOn: $settings.notificationsSoundEnabled)
        }
    }

    private var cronsPane: some View {
        settingsPanel(
            title: "Crons",
            subtitle: "Cron jobs configuration from web settings."
        ) {
            editor(
                title: "Cron Definitions",
                text: $settings.cronJobs,
                minHeight: 180
            )
        }
    }

    private var heartbeatPane: some View {
        settingsPanel(
            title: "Heartbeat",
            subtitle: "Heartbeat scheduler controls."
        ) {
            Toggle("Enable heartbeat", isOn: $settings.heartbeatEnabled)
            Stepper(
                "Interval: \(settings.heartbeatIntervalMinutes) min",
                value: $settings.heartbeatIntervalMinutes,
                in: 1 ... 120
            )
        }
    }

    private var securityPane: some View {
        settingsPanel(
            title: "Security",
            subtitle: "Authentication and credential controls."
        ) {
            Toggle("Require password login", isOn: $settings.requirePassword)
            Toggle("Enable passkeys", isOn: $settings.passkeysEnabled)
        }
    }

    private var tailscalePane: some View {
        settingsPanel(
            title: "Tailscale",
            subtitle: "Remote connectivity settings."
        ) {
            Toggle("Enable Tailscale", isOn: $settings.tailscaleEnabled)
            TextField("Hostname", text: $settings.tailscaleHostname)
        }
    }

    private var channelsPane: some View {
        settingsPanel(
            title: "Channels",
            subtitle: "Channel routing and sender policies."
        ) {
            editor(
                title: "Channel Rules",
                text: $settings.channelRules,
                minHeight: 180
            )
        }
    }

    private var hooksPane: some View {
        settingsPanel(
            title: "Hooks",
            subtitle: "Hook commands and trigger settings."
        ) {
            editor(
                title: "Hooks Config",
                text: $settings.hooksConfig,
                minHeight: 180
            )
        }
    }

    private var llmsPane: some View {
        settingsPanel(
            title: "LLMs",
            subtitle: "Provider, model, and auth values."
        ) {
            Picker("Provider", selection: $settings.llmProvider) {
                ForEach(settings.llmProviders, id: \.self) { provider in
                    Text(provider.capitalized).tag(provider)
                }
            }

            TextField("Model", text: $settings.llmModel)
            SecureField("API key", text: $settings.llmApiKey)
        }
    }

    private var mcpPane: some View {
        settingsPanel(
            title: "MCP",
            subtitle: "MCP server registrations and transport settings."
        ) {
            editor(
                title: "MCP Servers",
                text: $settings.mcpServers,
                minHeight: 180
            )
        }
    }

    private var skillsPane: some View {
        settingsPanel(
            title: "Skills",
            subtitle: "Enabled skill packs and repositories."
        ) {
            editor(
                title: "Skills",
                text: $settings.skills,
                minHeight: 180
            )
        }
    }

    private var voicePane: some View {
        settingsPanel(
            title: "Voice",
            subtitle: "TTS and STT provider configuration."
        ) {
            Toggle("Enable voice", isOn: $settings.voiceEnabled)
            Picker("Voice provider", selection: $settings.voiceProvider) {
                ForEach(settings.voiceProviders, id: \.self) { provider in
                    Text(provider.capitalized).tag(provider)
                }
            }
            SecureField("Voice API key", text: $settings.voiceApiKey)
        }
    }

    private var terminalPane: some View {
        settingsPanel(
            title: "Terminal",
            subtitle: "Host terminal exposure settings."
        ) {
            Toggle("Enable terminal tool", isOn: $settings.terminalEnabled)
            TextField("Default shell", text: $settings.terminalShell)
        }
    }

    private var sandboxesPane: some View {
        settingsPanel(
            title: "Sandboxes",
            subtitle: "Container backend and image defaults."
        ) {
            Picker("Backend", selection: $settings.sandboxBackend) {
                ForEach(settings.sandboxBackends, id: \.self) { backend in
                    Text(backend.capitalized).tag(backend)
                }
            }
            TextField("Default image", text: $settings.sandboxImage)
        }
    }

    private var monitoringPane: some View {
        settingsPanel(
            title: "Monitoring",
            subtitle: "Metrics and tracing controls."
        ) {
            Toggle("Enable monitoring", isOn: $settings.monitoringEnabled)
            Toggle("Enable metrics", isOn: $settings.metricsEnabled)
            Toggle("Enable tracing", isOn: $settings.tracingEnabled)
        }
    }

    private var logsPane: some View {
        settingsPanel(
            title: "Logs",
            subtitle: "Log level and persistence options."
        ) {
            Picker("Log level", selection: $settings.logLevel) {
                ForEach(settings.logLevels, id: \.self) { level in
                    Text(level.uppercased()).tag(level)
                }
            }
            Toggle("Persist logs to disk", isOn: $settings.persistLogs)
        }
    }

    private var graphqlPane: some View {
        settingsPanel(
            title: "GraphQL",
            subtitle: "GraphQL endpoint exposure."
        ) {
            Toggle("Enable GraphQL", isOn: $settings.graphqlEnabled)
            TextField("GraphQL path", text: $settings.graphqlPath)
        }
    }

    private var configurationPane: some View {
        settingsPanel(
            title: "Configuration",
            subtitle: "Raw TOML, consumed by Rust validation."
        ) {
            editor(
                title: "moltis.toml",
                text: $settings.configurationToml,
                minHeight: 320
            )
        }
    }

    private func settingsPanel<Content: View>(
        title: String,
        subtitle: String,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 6) {
                Text(title)
                    .font(.title3.weight(.semibold))
                Text(subtitle)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            content()
        }
    }

    private func editor(
        title: String,
        text: Binding<String>,
        minHeight: CGFloat
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.headline)

            TextEditor(text: text)
                .font(.system(.body, design: .monospaced))
                .frame(minHeight: minHeight)
                .overlay {
                    RoundedRectangle(cornerRadius: 8)
                        .stroke(.secondary.opacity(0.3), lineWidth: 1)
                }
        }
    }
}
