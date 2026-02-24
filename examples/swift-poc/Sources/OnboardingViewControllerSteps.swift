import AppKit

extension OnboardingViewController {
    func buildWelcomeStepView() -> NSView {
        let stack = makeVerticalStack(spacing: 12)

        let headline = NSTextField(labelWithString: "What this onboarding sets up")
        headline.font = .systemFont(ofSize: 18, weight: .semibold)
        stack.addArrangedSubview(headline)

        let bullets = [
            "1. Assistant identity and soul prompt.",
            "2. Default LLM provider and model.",
            "3. Optional voice provider defaults."
        ]

        bullets.forEach { text in
            let label = NSTextField(wrappingLabelWithString: text)
            label.font = .systemFont(ofSize: 14, weight: .regular)
            label.textColor = .labelColor
            label.maximumNumberOfLines = 2
            stack.addArrangedSubview(label)
        }

        stack.addArrangedSubview(NSView())
        return stack
    }

    func buildIdentityStepView() -> NSView {
        let stack = makeVerticalStack(spacing: 10)

        identityNameField.placeholderString = "Assistant display name"
        identityNameField.font = .systemFont(ofSize: 14, weight: .regular)

        let soulScroll = makeTextEditorScrollView(textView: soulPromptTextView, minHeight: 180)

        stack.addArrangedSubview(makeFieldLabel("Display Name"))
        stack.addArrangedSubview(identityNameField)
        stack.addArrangedSubview(makeFieldLabel("Soul Prompt"))
        stack.addArrangedSubview(soulScroll)

        return stack
    }

    func buildLlmStepView() -> NSView {
        let stack = makeVerticalStack(spacing: 10)

        modelField.placeholderString = "gpt-4.1"
        apiKeyField.placeholderString = "sk-..."

        stack.addArrangedSubview(makeFieldLabel("Provider"))
        stack.addArrangedSubview(providerPopup)
        stack.addArrangedSubview(makeFieldLabel("Model"))
        stack.addArrangedSubview(modelField)
        stack.addArrangedSubview(makeFieldLabel("API Key"))
        stack.addArrangedSubview(apiKeyField)

        return stack
    }

    func buildVoiceStepView() -> NSView {
        let stack = makeVerticalStack(spacing: 10)

        voiceApiKeyField.placeholderString = "Voice API key"

        stack.addArrangedSubview(voiceToggle)
        stack.addArrangedSubview(makeFieldLabel("Voice Provider"))
        stack.addArrangedSubview(voiceProviderPopup)
        stack.addArrangedSubview(makeFieldLabel("Voice API Key"))
        stack.addArrangedSubview(voiceApiKeyField)

        return stack
    }

    func buildReadyStepView() -> NSView {
        summaryLabel.font = .systemFont(ofSize: 14, weight: .regular)
        summaryLabel.textColor = .labelColor
        summaryLabel.maximumNumberOfLines = 0

        let tipText = "Tip: use Cmd-, at any time to reopen Settings."
        let tip = NSTextField(wrappingLabelWithString: tipText)
        tip.font = .systemFont(ofSize: 13, weight: .medium)
        tip.textColor = .secondaryLabelColor

        let stack = NSStackView(views: [summaryLabel, tip, NSView()])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 14
        return stack
    }

    func makeVerticalStack(spacing: CGFloat) -> NSStackView {
        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = spacing
        return stack
    }

    func makeFieldLabel(_ value: String) -> NSTextField {
        let label = NSTextField(labelWithString: value)
        label.font = .systemFont(ofSize: 12, weight: .semibold)
        label.textColor = .secondaryLabelColor
        return label
    }

    func makeTextEditorScrollView(textView: NSTextView, minHeight: CGFloat) -> NSScrollView {
        textView.font = .systemFont(ofSize: 13, weight: .regular)
        textView.textColor = .labelColor
        textView.backgroundColor = .clear
        textView.isRichText = false
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false

        let scroll = NSScrollView()
        scroll.borderType = .bezelBorder
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.documentView = textView
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: minHeight).isActive = true
        return scroll
    }
}
