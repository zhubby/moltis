import Foundation

enum OnboardingStep: Int, CaseIterable {
    case welcome
    case identity
    case llm
    case voice
    case ready

    var symbolName: String {
        switch self {
        case .welcome:
            return "sparkles"
        case .identity:
            return "person.crop.circle.fill"
        case .llm:
            return "cpu.fill"
        case .voice:
            return "waveform.circle.fill"
        case .ready:
            return "checkmark.seal.fill"
        }
    }

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
            return "Welcome to Moltis"
        case .identity:
            return "Assistant Identity"
        case .llm:
            return "Language Model"
        case .voice:
            return "Voice"
        case .ready:
            return "Ready to Go"
        }
    }

    var subtitle: String {
        switch self {
        case .welcome:
            return "Set up your assistant in a few quick steps."
        case .identity:
            return "Give your assistant a name and personality."
        case .llm:
            return "Choose your preferred model and provider."
        case .voice:
            return "Optionally enable voice interaction."
        case .ready:
            return "Everything looks good. You're all set."
        }
    }
}

enum RailStepState {
    case upcoming
    case current
    case done
}
