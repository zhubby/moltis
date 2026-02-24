import AppKit

final class OnboardingViewController: NSViewController {
    let settings: AppSettings
    let onFinish: () -> Void

    var currentStep = OnboardingStep.welcome
    var currentContentView: NSView?
    var pageDots: OnboardingPageDotsView!

    let heroSymbol = NSImageView()
    let titleLabel = NSTextField(labelWithString: "")
    let subtitleLabel = NSTextField(labelWithString: "")
    let validationLabel = NSTextField(labelWithString: "")
    let contentContainer = NSView()
    let backButton = NSButton(title: "Back", target: nil, action: nil)
    let nextButton = NSButton(title: "Continue", target: nil, action: nil)

    let identityNameField = NSTextField(string: "")
    let soulPromptTextView = NSTextView()
    let providerPopup = NSPopUpButton()
    let modelField = NSTextField(string: "")
    let apiKeyField = NSSecureTextField(string: "")
    let voiceToggle = NSButton(checkboxWithTitle: "Enable voice", target: nil, action: nil)
    let voiceProviderPopup = NSPopUpButton()
    let voiceApiKeyField = NSSecureTextField(string: "")

    var summaryRows: [OnboardingSummaryRow] = []

    lazy var welcomeStepView = buildWelcomeStepView()
    lazy var identityStepView = buildIdentityStepView()
    lazy var llmStepView = buildLlmStepView()
    lazy var voiceStepView = buildVoiceStepView()
    lazy var readyStepView = buildReadyStepView()

    init(settings: AppSettings, onFinish: @escaping () -> Void) {
        self.settings = settings
        self.onFinish = onFinish
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func loadView() {
        view = NSView()
        view.wantsLayer = true

        configureStaticControls()
        setupLayout()
        syncSettings()
        render(step: .welcome, animated: false)
    }

    func syncSettings() {
        identityNameField.stringValue = settings.identityName
        soulPromptTextView.string = settings.identitySoul
        modelField.stringValue = settings.llmModel
        apiKeyField.stringValue = settings.llmApiKey
        voiceToggle.state = settings.voiceEnabled ? .on : .off
        voiceApiKeyField.stringValue = settings.voiceApiKey
        refreshPopups()
    }
}

extension OnboardingViewController {
    @objc func handleBack() {
        guard let previous = OnboardingStep(rawValue: currentStep.rawValue - 1) else {
            return
        }
        render(step: previous, animated: true)
    }

    @objc func handleNext() {
        guard validateCurrentStep() else {
            NSSound.beep()
            return
        }
        persistCurrentStep()

        guard let next = OnboardingStep(rawValue: currentStep.rawValue + 1) else {
            onFinish()
            return
        }
        render(step: next, animated: true)
    }

    func render(step: OnboardingStep, animated: Bool) {
        currentStep = step
        titleLabel.stringValue = step.title
        subtitleLabel.stringValue = step.subtitle
        validationLabel.stringValue = ""
        backButton.isHidden = step == .welcome
        nextButton.title = step == .ready ? "Get Started" : "Continue"

        if let symbolImage = NSImage(
            systemSymbolName: step.symbolName,
            accessibilityDescription: step.title
        ) {
            heroSymbol.image = symbolImage
        }

        pageDots.update(currentIndex: step.rawValue)
        replaceContent(with: makeStepView(step), animated: animated)

        if step == .ready {
            updateSummary()
        }
    }

    func replaceContent(with nextView: NSView, animated: Bool) {
        if animated, let layer = contentContainer.layer {
            let transition = CATransition()
            transition.type = .fade
            transition.duration = 0.2
            transition.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            layer.add(transition, forKey: "onboarding-fade")
        }

        currentContentView?.removeFromSuperview()
        currentContentView = nextView
        nextView.translatesAutoresizingMaskIntoConstraints = false
        contentContainer.addSubview(nextView)
        NSLayoutConstraint.activate([
            nextView.leadingAnchor.constraint(equalTo: contentContainer.leadingAnchor),
            nextView.trailingAnchor.constraint(equalTo: contentContainer.trailingAnchor),
            nextView.topAnchor.constraint(equalTo: contentContainer.topAnchor),
            nextView.bottomAnchor.constraint(equalTo: contentContainer.bottomAnchor)
        ])
    }

    func validateCurrentStep() -> Bool {
        switch currentStep {
        case .identity:
            let trimmed = identityNameField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                validationLabel.stringValue = "Please provide an identity name."
                return false
            }
            return true
        case .llm:
            let trimmed = modelField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                validationLabel.stringValue = "Please provide a model name."
                return false
            }
            return true
        case .voice:
            let hasNoneProvider = voiceProviderPopup.titleOfSelectedItem == "none"
            if voiceToggle.state == .on && hasNoneProvider {
                validationLabel.stringValue = "Enable a voice provider or disable voice."
                return false
            }
            return true
        case .welcome, .ready:
            return true
        }
    }

    func persistCurrentStep() {
        switch currentStep {
        case .identity:
            settings.identityName = identityNameField.stringValue
            settings.identitySoul = soulPromptTextView.string
        case .llm:
            settings.llmProvider = providerPopup.titleOfSelectedItem?.lowercased() ?? settings.llmProvider
            settings.llmModel = modelField.stringValue
            settings.llmApiKey = apiKeyField.stringValue
        case .voice:
            settings.voiceEnabled = voiceToggle.state == .on
            settings.voiceProvider = voiceProviderPopup.titleOfSelectedItem?.lowercased() ?? settings.voiceProvider
            settings.voiceApiKey = voiceApiKeyField.stringValue
        case .welcome, .ready:
            break
        }
    }

    func makeStepView(_ step: OnboardingStep) -> NSView {
        switch step {
        case .welcome:
            return welcomeStepView
        case .identity:
            return identityStepView
        case .llm:
            return llmStepView
        case .voice:
            return voiceStepView
        case .ready:
            return readyStepView
        }
    }

    func refreshPopups() {
        providerPopup.removeAllItems()
        providerPopup.addItems(withTitles: settings.llmProviders)
        selectPopup(providerPopup, targetValue: settings.llmProvider)

        voiceProviderPopup.removeAllItems()
        voiceProviderPopup.addItems(withTitles: settings.voiceProviders)
        selectPopup(voiceProviderPopup, targetValue: settings.voiceProvider)
    }

    func selectPopup(_ popup: NSPopUpButton, targetValue: String) {
        if let index = popup.itemTitles.firstIndex(where: {
            $0.caseInsensitiveCompare(targetValue) == .orderedSame
        }) {
            popup.selectItem(at: index)
            return
        }
        popup.selectItem(at: 0)
    }

    func updateSummary() {
        let values = [
            settings.identityName,
            "\(settings.llmProvider) / \(settings.llmModel)",
            settings.voiceEnabled ? "Enabled (\(settings.voiceProvider))" : "Disabled"
        ]
        for (index, row) in summaryRows.enumerated() where index < values.count {
            if let valueField = row.viewWithTag(1) as? NSTextField {
                valueField.stringValue = values[index]
            }
        }
    }
}
