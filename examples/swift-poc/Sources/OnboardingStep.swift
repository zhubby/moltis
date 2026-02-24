import Foundation

enum OnboardingStep: Int, CaseIterable {
    case welcome
    case identity
    case llm
    case voice
    case ready

    var railLabel: String {
        switch self {
        case .welcome:
            return "Welcome"
        case .identity:
            return "Identity"
        case .llm:
            return "Model"
        case .voice:
            return "Voice"
        case .ready:
            return "Ready"
        }
    }

    var title: String {
        switch self {
        case .welcome:
            return "Welcome to Moltis for macOS"
        case .identity:
            return "Set your assistant identity"
        case .llm:
            return "Pick your LLM defaults"
        case .voice:
            return "Configure optional voice features"
        case .ready:
            return "You're ready to start chatting"
        }
    }

    var subtitle: String {
        switch self {
        case .welcome:
            return "A polished setup inspired by native AppKit apps."
        case .identity:
            return "This mirrors the Identity section from web settings."
        case .llm:
            return "Choose provider, model, and API key for this POC."
        case .voice:
            return "Voice settings are optional and can be changed later."
        case .ready:
            return "Review your setup, then jump into session bubbles."
        }
    }
}

enum RailStepState {
    case upcoming
    case current
    case done
}
