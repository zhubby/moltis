import Foundation

enum MoltisClientError: Error, LocalizedError {
    case nilResponsePointer
    case jsonEncodingFailed
    case invalidResponse(String)
    case bridgeError(code: String, message: String)

    var errorDescription: String? {
        switch self {
        case .nilResponsePointer:
            return "Rust bridge returned a null response pointer"
        case .jsonEncodingFailed:
            return "Failed to encode Swift request into JSON"
        case let .invalidResponse(payload):
            return "Rust bridge returned unexpected payload: \(payload)"
        case let .bridgeError(code, message):
            return "Rust bridge error [\(code)]: \(message)"
        }
    }
}

struct BridgeVersionPayload: Decodable {
    let bridgeVersion: String
    let moltisVersion: String
    let configDir: String

    enum CodingKeys: String, CodingKey {
        case bridgeVersion = "bridge_version"
        case moltisVersion = "moltis_version"
        case configDir = "config_dir"
    }
}

struct BridgeValidationPayload: Decodable {
    let errors: Int
    let warnings: Int
    let info: Int
    let hasErrors: Bool

    enum CodingKeys: String, CodingKey {
        case errors
        case warnings
        case info
        case hasErrors = "has_errors"
    }
}

struct BridgeChatPayload: Decodable {
    let reply: String
    let configDir: String
    let defaultSoul: String
    let validation: BridgeValidationPayload?

    enum CodingKeys: String, CodingKey {
        case reply
        case configDir = "config_dir"
        case defaultSoul = "default_soul"
        case validation
    }
}

private struct BridgeErrorEnvelope: Decodable {
    let error: BridgeErrorPayload
}

private struct BridgeErrorPayload: Decodable {
    let code: String
    let message: String
}

struct MoltisClient {
    private let decoder = JSONDecoder()

    func version() throws -> BridgeVersionPayload {
        let payload = try consumeCStringPointer(moltis_version())
        return try decode(payload, as: BridgeVersionPayload.self)
    }

    func chat(message: String, configToml: String?) throws -> BridgeChatPayload {
        let request = ChatRequest(message: message, configToml: configToml)
        let encoder = JSONEncoder()
        let data = try encoder.encode(request)

        guard let json = String(data: data, encoding: .utf8) else {
            throw MoltisClientError.jsonEncodingFailed
        }

        let payload = try json.withCString { cValue in
            try consumeCStringPointer(moltis_chat_json(cValue))
        }

        return try decode(payload, as: BridgeChatPayload.self)
    }

    private func decode<T: Decodable>(_ payload: String, as type: T.Type) throws -> T {
        let data = Data(payload.utf8)

        if let value = try? decoder.decode(T.self, from: data) {
            return value
        }

        if let bridgeError = try? decoder.decode(BridgeErrorEnvelope.self, from: data) {
            throw MoltisClientError.bridgeError(
                code: bridgeError.error.code,
                message: bridgeError.error.message
            )
        }

        throw MoltisClientError.invalidResponse(payload)
    }

    private func consumeCStringPointer(
        _ value: UnsafeMutablePointer<CChar>?
    ) throws -> String {
        guard let value else {
            throw MoltisClientError.nilResponsePointer
        }

        defer {
            moltis_free_string(value)
        }

        return String(cString: value)
    }
}

private struct ChatRequest: Encodable {
    let message: String
    let configToml: String?

    enum CodingKeys: String, CodingKey {
        case message
        case configToml = "config_toml"
    }
}
