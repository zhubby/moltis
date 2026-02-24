import AppKit

extension OnboardingViewController {
    func configureStaticControls() {
        titleLabel.font = .systemFont(ofSize: 26, weight: .bold)
        titleLabel.alignment = .center

        subtitleLabel.font = .systemFont(ofSize: 14, weight: .regular)
        subtitleLabel.textColor = .secondaryLabelColor
        subtitleLabel.alignment = .center
        subtitleLabel.maximumNumberOfLines = 2
        subtitleLabel.lineBreakMode = .byWordWrapping

        validationLabel.font = .systemFont(ofSize: 12, weight: .medium)
        validationLabel.textColor = .systemRed
        validationLabel.alignment = .center

        backButton.bezelStyle = .rounded
        backButton.controlSize = .large
        backButton.target = self
        backButton.action = #selector(handleBack)

        nextButton.bezelStyle = .rounded
        nextButton.controlSize = .large
        nextButton.keyEquivalent = "\r"
        nextButton.target = self
        nextButton.action = #selector(handleNext)
    }

    func setupLayout() {
        let bg = NSVisualEffectView()
        bg.blendingMode = .behindWindow
        bg.material = .underWindowBackground
        bg.state = .active
        bg.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(bg)

        NSLayoutConstraint.activate([
            bg.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            bg.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            bg.topAnchor.constraint(equalTo: view.topAnchor),
            bg.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])

        heroSymbol.translatesAutoresizingMaskIntoConstraints = false
        heroSymbol.contentTintColor = .controlAccentColor
        heroSymbol.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 48, weight: .light)
        heroSymbol.imageAlignment = .alignCenter

        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        subtitleLabel.translatesAutoresizingMaskIntoConstraints = false
        contentContainer.translatesAutoresizingMaskIntoConstraints = false
        contentContainer.wantsLayer = true
        validationLabel.translatesAutoresizingMaskIntoConstraints = false

        pageDots = OnboardingPageDotsView(count: OnboardingStep.allCases.count)

        let buttonStack = NSStackView(views: [backButton, nextButton])
        buttonStack.orientation = .horizontal
        buttonStack.spacing = 12

        let footer = NSStackView(views: [validationLabel, pageDots, buttonStack])
        footer.orientation = .vertical
        footer.alignment = .centerX
        footer.spacing = 16

        let topSpacer = NSView()
        topSpacer.translatesAutoresizingMaskIntoConstraints = false
        let midSpacer = NSView()
        midSpacer.translatesAutoresizingMaskIntoConstraints = false

        let mainStack = NSStackView(views: [
            topSpacer,
            heroSymbol,
            titleLabel,
            subtitleLabel,
            contentContainer,
            midSpacer,
            footer
        ])
        mainStack.orientation = .vertical
        mainStack.alignment = .centerX
        mainStack.spacing = 0
        mainStack.translatesAutoresizingMaskIntoConstraints = false
        bg.addSubview(mainStack)

        mainStack.setCustomSpacing(20, after: heroSymbol)
        mainStack.setCustomSpacing(6, after: titleLabel)
        mainStack.setCustomSpacing(28, after: subtitleLabel)

        NSLayoutConstraint.activate([
            mainStack.leadingAnchor.constraint(equalTo: bg.leadingAnchor, constant: 52),
            mainStack.trailingAnchor.constraint(equalTo: bg.trailingAnchor, constant: -52),
            mainStack.topAnchor.constraint(equalTo: bg.topAnchor, constant: 28),
            mainStack.bottomAnchor.constraint(equalTo: bg.bottomAnchor, constant: -24),

            contentContainer.widthAnchor.constraint(equalTo: mainStack.widthAnchor),
            contentContainer.heightAnchor.constraint(greaterThanOrEqualToConstant: 200),

            topSpacer.heightAnchor.constraint(equalToConstant: 4),
            midSpacer.heightAnchor.constraint(greaterThanOrEqualToConstant: 8),

            nextButton.widthAnchor.constraint(greaterThanOrEqualToConstant: 120),
            backButton.widthAnchor.constraint(greaterThanOrEqualToConstant: 80),

            view.widthAnchor.constraint(equalToConstant: 560),
            view.heightAnchor.constraint(greaterThanOrEqualToConstant: 500)
        ])
    }
}
