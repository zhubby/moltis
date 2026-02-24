import Combine
import Foundation

final class ChatStore: ObservableObject {
    @Published private(set) var sessions: [ChatSession]
    @Published var selectedSessionID: UUID?
    @Published var draftMessage = ""
    @Published var isSending = false
    @Published var statusText = "Ready"
    @Published var bridgeSummary = "Bridge metadata not loaded"

    private let client: MoltisClient
    let settings: AppSettings

    init(client: MoltisClient = MoltisClient(), settings: AppSettings) {
        self.client = client
        self.settings = settings

        let initialSession = ChatSession(
            title: "Session 1",
            messages: [ChatMessage(role: .system, text: "Session started.")]
        )
        sessions = [initialSession]
        selectedSessionID = initialSession.id
    }

    var selectedSession: ChatSession? {
        guard let selectedSessionID else {
            return nil
        }
        return sessions.first(where: { $0.id == selectedSessionID })
    }

    var selectedMessageAnchorID: UUID? {
        selectedSession?.messages.last?.id
    }

    func createSession() {
        let nextNumber = sessions.count + 1
        let session = ChatSession(
            title: "Session \(nextNumber)",
            messages: [ChatMessage(role: .system, text: "Session started.")]
        )
        sessions.insert(session, at: 0)
        selectedSessionID = session.id
    }

    func loadVersion() {
        do {
            let version = try client.version()
            bridgeSummary = "Bridge \(version.bridgeVersion) - Moltis \(version.moltisVersion)"
            settings.environmentConfigDir = version.configDir
            statusText = "Loaded version and config directory."
            appendMessage(
                role: .system,
                text: "Using config dir: \(version.configDir)"
            )
        } catch {
            let text = error.localizedDescription
            statusText = text
            appendMessage(role: .error, text: text)
        }
    }

    func sendDraftMessage() {
        guard !isSending else {
            return
        }

        let trimmed = draftMessage.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return
        }

        if selectedSessionID == nil {
            createSession()
        }

        appendMessage(role: .user, text: trimmed)
        updateSessionTitleIfNeeded(with: trimmed)
        draftMessage = ""

        let rawConfig = settings.configurationToml.trimmingCharacters(
            in: .whitespacesAndNewlines
        )
        let configToml: String? = rawConfig.isEmpty ? nil : rawConfig

        isSending = true
        do {
            let payload = try client.chat(message: trimmed, configToml: configToml)

            settings.environmentConfigDir = payload.configDir
            settings.identitySoul = payload.defaultSoul

            appendMessage(role: .assistant, text: payload.reply)
            appendValidationSummary(payload.validation)
            statusText = "Received Rust response."
        } catch {
            let text = error.localizedDescription
            statusText = text
            appendMessage(role: .error, text: text)
        }
        isSending = false
    }

    private func appendValidationSummary(_ validation: BridgeValidationPayload?) {
        guard let validation else {
            return
        }

        let summary =
            "Validation: \(validation.errors) errors, \(validation.warnings) warnings, " +
            "\(validation.info) info."
        let role: ChatMessageRole = validation.hasErrors ? .error : .system
        appendMessage(role: role, text: summary)
    }

    private func appendMessage(role: ChatMessageRole, text: String) {
        guard let index = selectedSessionIndex() else {
            return
        }

        var session = sessions[index]
        session.messages.append(ChatMessage(role: role, text: text))
        session.updatedAt = Date()
        sessions[index] = session
    }

    private func selectedSessionIndex() -> Int? {
        guard let selectedSessionID else {
            return nil
        }
        return sessions.firstIndex(where: { $0.id == selectedSessionID })
    }

    private func updateSessionTitleIfNeeded(with message: String) {
        guard let index = selectedSessionIndex() else {
            return
        }

        var session = sessions[index]
        guard session.title.hasPrefix("Session ") else {
            return
        }

        let compact = message
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: "\n", with: " ")
        let shortTitle = String(compact.prefix(24))
        if !shortTitle.isEmpty {
            session.title = shortTitle
            sessions[index] = session
        }
    }
}
