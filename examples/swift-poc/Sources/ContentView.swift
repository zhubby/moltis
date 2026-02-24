import SwiftUI

struct ContentView: View {
    @ObservedObject var chatStore: ChatStore
    @ObservedObject var settings: AppSettings
    @Environment(\.openSettings) private var openSettings

    var body: some View {
        NavigationSplitView {
            SessionsSidebarView(chatStore: chatStore)
        } detail: {
            ChatDetailView(chatStore: chatStore, settings: settings) {
                openSettings()
            }
        }
        .navigationSplitViewStyle(.balanced)
        .frame(minWidth: 1080, minHeight: 720)
    }
}

#Preview {
    let settings = AppSettings()
    let store = ChatStore(settings: settings)
    return ContentView(chatStore: store, settings: settings)
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

            List(selection: $chatStore.selectedSessionID) {
                ForEach(chatStore.sessions) { session in
                    SessionRowView(session: session)
                        .tag(Optional(session.id))
                }
            }
            .listStyle(.sidebar)
        }
        .padding(.top, 12)
    }
}

private struct SessionRowView: View {
    let session: ChatSession

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(session.title)
                .font(.headline)
                .lineLimit(1)

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
            HStack(alignment: .top, spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(sessionTitle)
                        .font(.title3.weight(.semibold))
                    Text(chatStore.bridgeSummary)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()

                Button("Load Version") {
                    chatStore.loadVersion()
                }

                Button("Settings") {
                    openSettings()
                }
            }
            .padding(16)

            Divider()

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
                .background(Color(nsColor: .controlBackgroundColor))
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

            Divider()

            VStack(alignment: .leading, spacing: 8) {
                Text(chatStore.statusText)
                    .font(.caption)
                    .foregroundStyle(.secondary)

                Text("Config source: Settings > Configuration")
                    .font(.caption2)
                    .foregroundStyle(.secondary)

                HStack(alignment: .bottom, spacing: 10) {
                    TextField(
                        "Send a message to Rust bridge...",
                        text: $chatStore.draftMessage,
                        axis: .vertical
                    )
                    .lineLimit(1 ... 4)
                    .textFieldStyle(.roundedBorder)

                    Button(chatStore.isSending ? "Sending..." : "Send") {
                        chatStore.sendDraftMessage()
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!canSendMessage)
                }
            }
            .padding(16)
        }
    }
}

private struct MessageBubbleView: View {
    let message: ChatMessage

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
                Text(message.role.title)
                    .font(.caption2)
                    .foregroundStyle(.secondary)

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
