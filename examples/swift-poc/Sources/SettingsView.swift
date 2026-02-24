import SwiftUI

enum SettingsGroup: String, CaseIterable, Hashable {
    case general = "General"
    case security = "Security"
    case integrations = "Integrations"
    case systems = "Systems"
}

enum SettingsSection: String, CaseIterable, Hashable {
    case identity = "Identity"
    case environment = "Environment"
    case memory = "Memory"
    case notifications = "Notifications"
    case crons = "Crons"
    case heartbeat = "Heartbeat"
    case security = "Security"
    case tailscale = "Tailscale"
    case channels = "Channels"
    case hooks = "Hooks"
    case llms = "LLMs"
    case mcp = "MCP"
    case skills = "Skills"
    case voice = "Voice"
    case terminal = "Terminal"
    case sandboxes = "Sandboxes"
    case monitoring = "Monitoring"
    case logs = "Logs"
    case graphql = "GraphQL"
    case configuration = "Configuration"

    var title: String {
        rawValue
    }

    var icon: String {
        Self.iconMap[self] ?? "gearshape"
    }

    var group: SettingsGroup {
        Self.groupMap[self] ?? .general
    }

    private static let iconMap: [SettingsSection: String] = [
        .identity: "person.crop.circle",
        .environment: "terminal",
        .memory: "externaldrive",
        .notifications: "bell",
        .crons: "clock.arrow.circlepath",
        .heartbeat: "heart.text.square",
        .security: "lock.shield",
        .tailscale: "network",
        .channels: "point.3.connected.trianglepath.dotted",
        .hooks: "wrench.and.screwdriver",
        .llms: "square.stack.3d.down.forward",
        .mcp: "link",
        .skills: "sparkles",
        .voice: "mic",
        .terminal: "apple.terminal",
        .sandboxes: "shippingbox",
        .monitoring: "chart.bar",
        .logs: "doc.plaintext",
        .graphql: "dot.squareshape.split.2x2",
        .configuration: "slider.horizontal.3"
    ]

    private static let groupMap: [SettingsSection: SettingsGroup] = [
        .identity: .general,
        .environment: .general,
        .memory: .general,
        .notifications: .general,
        .crons: .general,
        .heartbeat: .general,
        .security: .security,
        .tailscale: .security,
        .channels: .integrations,
        .hooks: .integrations,
        .llms: .integrations,
        .mcp: .integrations,
        .skills: .integrations,
        .voice: .integrations,
        .terminal: .systems,
        .sandboxes: .systems,
        .monitoring: .systems,
        .logs: .systems,
        .graphql: .systems,
        .configuration: .systems
    ]
}

struct SettingsView: View {
    @ObservedObject var settings: AppSettings
    @State private var selectedSection = SettingsSection.identity

    var body: some View {
        NavigationSplitView {
            List(selection: $selectedSection) {
                ForEach(SettingsGroup.allCases, id: \.self) { group in
                    Section(group.rawValue) {
                        ForEach(sections(for: group), id: \.self) { section in
                            Label(section.title, systemImage: section.icon)
                                .tag(section)
                        }
                    }
                }
            }
            .navigationTitle("Settings")
            .navigationSplitViewColumnWidth(min: 240, ideal: 260)
        } detail: {
            ScrollView {
                SettingsSectionContent(
                    section: selectedSection,
                    settings: settings
                )
                .padding(20)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .frame(minWidth: 980, minHeight: 700)
    }

    private func sections(for group: SettingsGroup) -> [SettingsSection] {
        SettingsSection.allCases.filter { $0.group == group }
    }
}
