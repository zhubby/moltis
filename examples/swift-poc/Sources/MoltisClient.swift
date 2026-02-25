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

// MARK: - Version

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

// MARK: - Validation

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

// MARK: - Chat

struct BridgeChatPayload: Decodable {
    let reply: String
    let model: String?
    let provider: String?
    let configDir: String
    let defaultSoul: String
    let validation: BridgeValidationPayload?

    enum CodingKeys: String, CodingKey {
        case reply
        case model
        case provider
        case configDir = "config_dir"
        case defaultSoul = "default_soul"
        case validation
    }
}

// MARK: - Provider types

struct BridgeKnownProvider: Decodable, Identifiable {
    let name: String
    let displayName: String
    let authType: String
    let envKey: String?
    let defaultBaseUrl: String?
    let requiresModel: Bool
    let keyOptional: Bool

    var id: String { name }

    enum CodingKeys: String, CodingKey {
        case name
        case displayName = "display_name"
        case authType = "auth_type"
        case envKey = "env_key"
        case defaultBaseUrl = "default_base_url"
        case requiresModel = "requires_model"
        case keyOptional = "key_optional"
    }
}

struct BridgeDetectedSource: Decodable {
    let provider: String
    let source: String
}

struct BridgeModelInfo: Decodable, Identifiable {
    let id: String
    let provider: String
    let displayName: String
    let createdAt: Int?

    enum CodingKeys: String, CodingKey {
        case id
        case provider
        case displayName = "display_name"
        case createdAt = "created_at"
    }
}

// MARK: - Ok response

private struct BridgeOkPayload: Decodable {
    let ok: Bool
}

// MARK: - Error envelope

private struct BridgeErrorEnvelope: Decodable {
    let error: BridgeErrorPayload
}

private struct BridgeErrorPayload: Decodable {
    let code: String
    let message: String
}

// MARK: - Client

struct MoltisClient {
    private let decoder = JSONDecoder()

    func version() throws -> BridgeVersionPayload {
        let payload = try consumeCStringPointer(moltis_version())
        return try decode(payload, as: BridgeVersionPayload.self)
    }

    func chat(
        message: String,
        model: String? = nil,
        provider: String? = nil,
        configToml: String? = nil
    ) throws -> BridgeChatPayload {
        let request = ChatRequest(
            message: message,
            model: model,
            provider: provider,
            configToml: configToml
        )
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

    func knownProviders() throws -> [BridgeKnownProvider] {
        let payload = try consumeCStringPointer(moltis_known_providers())
        return try decode(payload, as: [BridgeKnownProvider].self)
    }

    func detectProviders() throws -> [BridgeDetectedSource] {
        let payload = try consumeCStringPointer(moltis_detect_providers())
        return try decode(payload, as: [BridgeDetectedSource].self)
    }

    func saveProviderConfig(
        provider: String,
        apiKey: String?,
        baseUrl: String?,
        models: [String]?
    ) throws {
        let request = SaveProviderRequest(
            provider: provider,
            apiKey: apiKey,
            baseUrl: baseUrl,
            models: models
        )
        let encoder = JSONEncoder()
        let data = try encoder.encode(request)

        guard let json = String(data: data, encoding: .utf8) else {
            throw MoltisClientError.jsonEncodingFailed
        }

        let payload = try json.withCString { cValue in
            try consumeCStringPointer(moltis_save_provider_config(cValue))
        }

        _ = try decode(payload, as: BridgeOkPayload.self)
    }

    func listModels() throws -> [BridgeModelInfo] {
        let payload = try consumeCStringPointer(moltis_list_models())
        return try decode(payload, as: [BridgeModelInfo].self)
    }

    func refreshRegistry() throws {
        let payload = try consumeCStringPointer(moltis_refresh_registry())
        _ = try decode(payload, as: BridgeOkPayload.self)
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

// MARK: - Request types

private struct ChatRequest: Encodable {
    let message: String
    let model: String?
    let provider: String?
    let configToml: String?

    enum CodingKeys: String, CodingKey {
        case message
        case model
        case provider
        case configToml = "config_toml"
    }
}

private struct SaveProviderRequest: Encodable {
    let provider: String
    let apiKey: String?
    let baseUrl: String?
    let models: [String]?

    enum CodingKeys: String, CodingKey {
        case provider
        case apiKey = "api_key"
        case baseUrl = "base_url"
        case models
    }
}
