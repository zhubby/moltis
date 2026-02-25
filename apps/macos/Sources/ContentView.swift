import SwiftUI

struct ContentView: View {
    @ObservedObject var chatStore: ChatStore
    @ObservedObject var settings: AppSettings
    @ObservedObject var providerStore: ProviderStore
    @Environment(\.openSettings) private var openSettings

    var body: some View {
        NavigationSplitView {
            SessionsSidebarView(chatStore: chatStore)
        } detail: {
            ChatDetailView(
                chatStore: chatStore,
                settings: settings,
                providerStore: providerStore
            ) {
                openSettings()
            }
        }
        .navigationSplitViewStyle(.balanced)
        .frame(minWidth: 1080, minHeight: 720)
    }
}

#Preview {
    let settings = AppSettings()
    let providerStore = ProviderStore()
    let store = ChatStore(settings: settings, providerStore: providerStore)
    return ContentView(chatStore: store, settings: settings, providerStore: providerStore)
}

private struct SessionsSidebarView: View {
    @ObservedObject var chatStore: ChatStore

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Sessions")
                    .font(.title3.weight(.semibold))
                Spacer()
                Button {
                    chatStore.createSession()
                } label: {
                    Image(systemName: "plus")
                }
                .buttonStyle(.borderless)
                .help("Create a new session")
            }
            .padding(.horizontal, 12)

            ScrollViewReader { proxy in
                List(selection: $chatStore.selectedSessionID) {
                    ForEach(chatStore.sessions) { session in
                        SessionRowView(session: session)
                            .tag(Optional(session.id))
                            .id(session.id)
                    }
                }
                .listStyle(.sidebar)
                .onChange(of: chatStore.selectedSessionID) { _, newID in
                    guard let newID else { return }
                    withAnimation(.easeInOut(duration: 0.35)) {
                        proxy.scrollTo(newID, anchor: .top)
                    }
                }
            }
        }
        .padding(.top, 12)
    }
}

private struct SessionRowView: View {
    let session: ChatSession

    private static let timeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(session.title)
                    .font(.headline)
                    .lineLimit(1)
                Spacer()
                Text(Self.timeFormatter.string(from: session.updatedAt))
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }

            Text(session.previewText)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
        .padding(.vertical, 4)
    }
}

private struct ChatDetailView: View {
    @ObservedObject var chatStore: ChatStore
    @ObservedObject var settings: AppSettings
    @ObservedObject var providerStore: ProviderStore
    var openSettings: () -> Void

    private var sessionTitle: String {
        chatStore.selectedSession?.title ?? "No Session Selected"
    }

    private var sessionMessages: [ChatMessage] {
        chatStore.selectedSession?.messages ?? []
    }

    private var canSendMessage: Bool {
        let trimmed = chatStore.draftMessage.trimmingCharacters(
            in: .whitespacesAndNewlines
        )
        return !trimmed.isEmpty && !chatStore.isSending
    }

    @State private var showContextPopover = false

    var body: some View {
        VStack(spacing: 0) {
            headerBar

            sessionToolbar

            Divider()

            messageList

            if settings.debugEnabled {
                Divider()
                debugPanel
            }

            Divider()

            inputBar
        }
    }

    private var headerBar: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(sessionTitle)
                    .font(.title3.weight(.semibold))
                HStack(spacing: 6) {
                    Text(chatStore.bridgeSummary)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    if let modelSummary = providerStore.selectedModelSummary {
                        Text("| \(modelSummary)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }

            Spacer()

            Button {
                openSettings()
            } label: {
                Image(systemName: "gearshape")
            }
            .buttonStyle(.borderless)
            .help("Settings")
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private var sessionToolbar: some View {
        HStack(spacing: 12) {
            // Model picker
            if !providerStore.models.isEmpty {
                Picker("Model", selection: $providerStore.selectedModelID) {
                    Text("Default").tag(nil as String?)
                    ForEach(providerStore.models) { model in
                        Text("\(model.displayName) (\(model.provider))")
                            .tag(Optional(model.id))
                    }
                }
                .labelsHidden()
                .controlSize(.small)
                .frame(maxWidth: 240)
            }

            Divider()
                .frame(height: 16)

            // Sandbox toggle
            Toggle("Sandbox", isOn: $settings.sandboxEnabled)
                .toggleStyle(.switch)
                .controlSize(.small)

            if settings.sandboxEnabled {
                TextField("Image", text: $settings.containerImage)
                    .textFieldStyle(.roundedBorder)
                    .controlSize(.small)
                    .frame(maxWidth: 160)
            }

            Divider()
                .frame(height: 16)

            // Debug toggle
            Toggle("Debug", isOn: $settings.debugEnabled)
                .toggleStyle(.switch)
                .controlSize(.small)

            // Context button
            Button {
                showContextPopover.toggle()
            } label: {
                Image(systemName: "doc.text.magnifyingglass")
            }
            .buttonStyle(.borderless)
            .controlSize(.small)
            .help("Session context")
            .popover(isPresented: $showContextPopover) {
                contextPopoverContent
            }

            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 6)
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private var contextPopoverContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Session Context")
                .font(.headline)

            LabeledContent("Config Dir", value: settings.environmentConfigDir.isEmpty ? "—" : settings.environmentConfigDir)
            LabeledContent("Provider", value: settings.llmProvider)
            LabeledContent("Model", value: providerStore.selectedModelID ?? "Default")

            if let session = chatStore.selectedSession {
                let totalIn = session.messages.compactMap(\.inputTokens).reduce(0, +)
                let totalOut = session.messages.compactMap(\.outputTokens).reduce(0, +)
                LabeledContent("Input Tokens", value: "\(totalIn)")
                LabeledContent("Output Tokens", value: "\(totalOut)")
            }
        }
        .padding()
        .frame(minWidth: 280)
    }

    private var debugPanel: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Debug")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)

            if let lastAssistant = sessionMessages.last(where: { $0.role == .assistant }) {
                Text("Last response: \(lastAssistant.provider ?? "?") / \(lastAssistant.model ?? "?")")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                if let inTok = lastAssistant.inputTokens, let outTok = lastAssistant.outputTokens {
                    Text("Tokens: \(inTok) in / \(outTok) out")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                if let ms = lastAssistant.durationMs {
                    Text("Duration: \(ms)ms")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            } else {
                Text("No assistant messages yet")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }

            Text("Bridge: \(chatStore.bridgeSummary)")
                .font(.caption2)
                .foregroundStyle(.secondary)
            Text("Config: \(settings.environmentConfigDir)")
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color(nsColor: .controlBackgroundColor).opacity(0.5))
    }

    private var messageList: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 10) {
                    ForEach(sessionMessages) { message in
                        MessageBubbleView(message: message)
                            .id(message.id)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(16)
            }
            .background {
                VisualEffectBackground(material: .underPageBackground)
            }
            .onAppear {
                if let anchor = chatStore.selectedMessageAnchorID {
                    proxy.scrollTo(anchor, anchor: .bottom)
                }
            }
            .onChange(of: chatStore.selectedMessageAnchorID) { _, anchor in
                guard let anchor else {
                    return
                }
                withAnimation(.easeOut(duration: 0.2)) {
                    proxy.scrollTo(anchor, anchor: .bottom)
                }
            }
        }
    }

    private var inputBar: some View {
        HStack(alignment: .center, spacing: 10) {
            ChatInputField(
                text: $chatStore.draftMessage,
                onSend: { chatStore.sendDraftMessage() }
            )
            .frame(minHeight: 28, maxHeight: 96)
            .background(Color(nsColor: .controlBackgroundColor))
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .overlay {
                RoundedRectangle(cornerRadius: 8)
                    .stroke(.quaternary, lineWidth: 1)
            }

            Button {
                chatStore.sendDraftMessage()
            } label: {
                Image(systemName: chatStore.isSending ? "ellipsis.circle.fill" : "arrow.up.circle.fill")
                    .font(.system(size: 28))
                    .foregroundStyle(canSendMessage ? .blue : .secondary.opacity(0.4))
            }
            .buttonStyle(.borderless)
            .disabled(!canSendMessage)
            .help("Send message")
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }
}

private struct MessageBubbleView: View {
    let message: ChatMessage

    private static let timeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter
    }()

    private var isUser: Bool {
        message.role == .user
    }

    private var bubbleFill: Color {
        switch message.role {
        case .user:
            return .accentColor.opacity(0.2)
        case .assistant:
            return Color(nsColor: .textBackgroundColor)
        case .system:
            return .yellow.opacity(0.18)
        case .error:
            return .red.opacity(0.18)
        }
    }

    private var bubbleBorder: Color {
        switch message.role {
        case .user:
            return .accentColor.opacity(0.4)
        case .assistant:
            return .secondary.opacity(0.25)
        case .system:
            return .yellow.opacity(0.5)
        case .error:
            return .red.opacity(0.5)
        }
    }

    private var metadataText: String? {
        guard message.role == .assistant else { return nil }

        var parts: [String] = []

        if let provider = message.provider {
            if let model = message.model {
                parts.append("\(provider) / \(model)")
            } else {
                parts.append(provider)
            }
        }

        if let inTok = message.inputTokens, let outTok = message.outputTokens {
            parts.append("\(inTok) in / \(outTok) out")
        }

        if let outTok = message.outputTokens, let ms = message.durationMs, ms > 0 {
            let tokPerSec = Double(outTok) / (Double(ms) / 1000.0)
            parts.append(String(format: "%.1f tok/s", tokPerSec))
        }

        return parts.isEmpty ? nil : parts.joined(separator: " \u{00B7} ")
    }

    private func speedColor(for message: ChatMessage) -> Color {
        guard let outTok = message.outputTokens, let ms = message.durationMs, ms > 0 else {
            return .secondary
        }
        let tokPerSec = Double(outTok) / (Double(ms) / 1000.0)
        if tokPerSec >= 25 { return .green }
        if tokPerSec < 10 { return .red }
        return .secondary
    }

    var body: some View {
        HStack {
            if isUser {
                Spacer(minLength: 80)
            }

            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text(message.role.title)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                    Spacer()
                    Text(Self.timeFormatter.string(from: message.createdAt))
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }

                Text(message.text)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)

                if let metadata = metadataText {
                    Text(metadata)
                        .font(.caption2)
                        .foregroundStyle(speedColor(for: message))
                        .frame(maxWidth: .infinity, alignment: .trailing)
                }
            }
            .padding(10)
            .frame(maxWidth: 640, alignment: .leading)
            .background(bubbleFill)
            .overlay {
                RoundedRectangle(cornerRadius: 12)
                    .stroke(bubbleBorder, lineWidth: 1)
            }
            .clipShape(RoundedRectangle(cornerRadius: 12))

            if !isUser {
                Spacer(minLength: 80)
            }
        }
        .frame(maxWidth: .infinity, alignment: isUser ? .trailing : .leading)
    }
}
