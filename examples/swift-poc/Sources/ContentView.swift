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

    var body: some View {
        VStack(spacing: 0) {
            headerBar

            Divider()

            messageList

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
            TextField(
                "Message...",
                text: $chatStore.draftMessage,
                axis: .vertical
            )
            .lineLimit(1 ... 4)
            .textFieldStyle(.plain)
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
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
