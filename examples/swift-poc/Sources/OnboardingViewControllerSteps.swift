import AppKit

extension OnboardingViewController {
    func buildWelcomeStepView() -> NSView {
        let stack = makeVerticalStack(spacing: 16)

        let features = [
            OnboardingFeatureRow(
                symbolName: "person.crop.circle.fill",
                title: "Identity",
                detail: "Name your assistant and set its personality."
            ),
            OnboardingFeatureRow(
                symbolName: "cpu.fill",
                title: "Language Model",
                detail: "Connect to your preferred LLM provider."
            ),
            OnboardingFeatureRow(
                symbolName: "waveform.circle.fill",
                title: "Voice",
                detail: "Optionally enable voice interaction."
            )
        ]

        features.forEach { stack.addArrangedSubview($0) }
        stack.addArrangedSubview(NSView())
        return stack
    }

    func buildIdentityStepView() -> NSView {
        let stack = makeVerticalStack(spacing: 8)

        identityNameField.placeholderString = "Assistant display name"
        identityNameField.font = .systemFont(ofSize: 14, weight: .regular)
        identityNameField.controlSize = .large

        let soulScroll = makeTextEditorScrollView(textView: soulPromptTextView, minHeight: 140)

        stack.addArrangedSubview(makeFieldLabel("Display Name"))
        stack.addArrangedSubview(identityNameField)
        stack.setCustomSpacing(16, after: identityNameField)
        stack.addArrangedSubview(makeFieldLabel("Soul Prompt"))
        stack.addArrangedSubview(soulScroll)

        return stack
    }

    func buildLlmStepView() -> NSView {
        let stack = makeVerticalStack(spacing: 8)

        providerPopup.controlSize = .large
        modelField.placeholderString = "gpt-4.1"
        modelField.controlSize = .large
        apiKeyField.placeholderString = "sk-..."
        apiKeyField.controlSize = .large

        stack.addArrangedSubview(makeFieldLabel("Provider"))
        stack.addArrangedSubview(providerPopup)
        stack.setCustomSpacing(16, after: providerPopup)
        stack.addArrangedSubview(makeFieldLabel("Model"))
        stack.addArrangedSubview(modelField)
        stack.setCustomSpacing(16, after: modelField)
        stack.addArrangedSubview(makeFieldLabel("API Key"))
        stack.addArrangedSubview(apiKeyField)

        return stack
    }

    func buildVoiceStepView() -> NSView {
        let stack = makeVerticalStack(spacing: 8)

        voiceToggle.controlSize = .large
        voiceProviderPopup.controlSize = .large
        voiceApiKeyField.placeholderString = "Voice API key"
        voiceApiKeyField.controlSize = .large

        stack.addArrangedSubview(voiceToggle)
        stack.setCustomSpacing(16, after: voiceToggle)
        stack.addArrangedSubview(makeFieldLabel("Voice Provider"))
        stack.addArrangedSubview(voiceProviderPopup)
        stack.setCustomSpacing(16, after: voiceProviderPopup)
        stack.addArrangedSubview(makeFieldLabel("Voice API Key"))
        stack.addArrangedSubview(voiceApiKeyField)

        return stack
    }

    func buildReadyStepView() -> NSView {
        let stack = makeVerticalStack(spacing: 16)

        let card = NSView()
        card.wantsLayer = true
        card.layer?.cornerRadius = 10
        card.layer?.backgroundColor = NSColor.controlBackgroundColor.cgColor
        card.layer?.borderColor = NSColor.separatorColor.withAlphaComponent(0.4).cgColor
        card.layer?.borderWidth = 0.5
        card.translatesAutoresizingMaskIntoConstraints = false

        let labels = ["Identity", "Model", "Voice"]
        let placeholders = ["—", "—", "—"]
        let cardStack = makeVerticalStack(spacing: 8)
        cardStack.translatesAutoresizingMaskIntoConstraints = false

        summaryRows = []
        for (index, label) in labels.enumerated() {
            let row = OnboardingSummaryRow(label: label, value: placeholders[index])
            summaryRows.append(row)
            cardStack.addArrangedSubview(row)
        }

        card.addSubview(cardStack)
        NSLayoutConstraint.activate([
            cardStack.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 16),
            cardStack.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -16),
            cardStack.topAnchor.constraint(equalTo: card.topAnchor, constant: 14),
            cardStack.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -14)
        ])

        let tip = NSTextField(wrappingLabelWithString: "You can change these anytime in Settings (\u{2318},).")
        tip.font = .systemFont(ofSize: 12, weight: .regular)
        tip.textColor = .tertiaryLabelColor

        stack.addArrangedSubview(card)
        stack.addArrangedSubview(tip)
        stack.addArrangedSubview(NSView())

        card.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true

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
        label.font = .systemFont(ofSize: 12, weight: .medium)
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
