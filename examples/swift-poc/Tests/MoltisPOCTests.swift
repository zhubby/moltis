@testable import MoltisPOC
import XCTest

final class MoltisPOCTests: XCTestCase {
    func testVersionPayloadDecodesCoreFields() throws {
        let client = MoltisClient()
        let payload = try client.version()

        XCTAssertFalse(payload.bridgeVersion.isEmpty)
        XCTAssertFalse(payload.moltisVersion.isEmpty)
        XCTAssertFalse(payload.configDir.isEmpty)
    }

    func testChatPayloadReturnsEchoedMessageAndValidation() throws {
        let client = MoltisClient()
        let payload = try client.chat(
            message: "swift test",
            configToml: "[server]\nport = \"invalid\""
        )

        XCTAssertTrue(payload.reply.contains("swift test"))
        XCTAssertNotNil(payload.validation)
        XCTAssertTrue(payload.validation?.hasErrors ?? false)
    }

    func testChatStoreAppendsAssistantAndValidationMessages() throws {
        let settings = AppSettings()
        settings.configurationToml = "[server]\nport = \"invalid\""

        let store = ChatStore(settings: settings)
        store.draftMessage = "store integration test"
        store.sendDraftMessage()

        let selectedSession = try XCTUnwrap(store.selectedSession)

        XCTAssertTrue(selectedSession.messages.contains(where: {
            $0.role == .user && $0.text.contains("store integration test")
        }))

        XCTAssertTrue(selectedSession.messages.contains(where: {
            $0.role == .assistant && $0.text.contains("store integration test")
        }))

        XCTAssertTrue(selectedSession.messages.contains(where: {
            $0.role == .error && $0.text.contains("Validation:")
        }))
    }
}
