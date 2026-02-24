import AppKit

extension OnboardingViewController {
    func configureStaticControls() {
        titleLabel.font = .systemFont(ofSize: 30, weight: .semibold)

        subtitleLabel.font = .systemFont(ofSize: 13, weight: .regular)
        subtitleLabel.textColor = .secondaryLabelColor
        subtitleLabel.maximumNumberOfLines = 2
        subtitleLabel.lineBreakMode = .byWordWrapping

        validationLabel.font = .systemFont(ofSize: 12, weight: .medium)
        validationLabel.textColor = .systemRed

        backButton.bezelStyle = .rounded
        backButton.target = self
        backButton.action = #selector(handleBack)

        nextButton.bezelStyle = .rounded
        nextButton.keyEquivalent = "\r"
        nextButton.target = self
        nextButton.action = #selector(handleNext)
    }

    func setupLayout() {
        let splitView = makeSplitView()
        view.addSubview(splitView)

        NSLayoutConstraint.activate([
            splitView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            splitView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            splitView.topAnchor.constraint(equalTo: view.topAnchor),
            splitView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])

        let sidebar = buildSidebar()
        let mainPanel = buildMainPanel()

        splitView.addArrangedSubview(sidebar)
        splitView.addArrangedSubview(mainPanel)
        sidebar.widthAnchor.constraint(equalToConstant: 260).isActive = true
    }

    func makeSplitView() -> NSStackView {
        let splitView = NSStackView()
        splitView.orientation = .horizontal
        splitView.alignment = .centerY
        splitView.distribution = .fill
        splitView.translatesAutoresizingMaskIntoConstraints = false
        return splitView
    }

    func buildSidebar() -> NSView {
        let sidebar = OnboardingGradientView()
        sidebar.translatesAutoresizingMaskIntoConstraints = false

        let stack = makeSidebarContentStack(in: sidebar)
        addSidebarBranding(to: stack)
        addSidebarRail(to: stack)
        addSidebarHint(to: stack)

        return sidebar
    }

    func makeSidebarContentStack(in sidebar: NSView) -> NSStackView {
        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 18
        stack.translatesAutoresizingMaskIntoConstraints = false
        sidebar.addSubview(stack)

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor, constant: 22),
            stack.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor, constant: -20),
            stack.topAnchor.constraint(equalTo: sidebar.topAnchor, constant: 24),
            stack.bottomAnchor.constraint(lessThanOrEqualTo: sidebar.bottomAnchor, constant: -24)
        ])
        return stack
    }

    func addSidebarBranding(to stack: NSStackView) {
        if let icon = NSImage(
            systemSymbolName: "sparkles.rectangle.stack.fill",
            accessibilityDescription: nil
        ) {
            let iconView = NSImageView(image: icon)
            iconView.contentTintColor = NSColor(calibratedRed: 0.95, green: 0.97, blue: 1.0, alpha: 1)
            iconView.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 30, weight: .bold)
            stack.addArrangedSubview(iconView)
        }

        let title = NSTextField(labelWithString: "Moltis Swift POC")
        title.font = .systemFont(ofSize: 20, weight: .semibold)
        title.textColor = .white

        let subtitle = NSTextField(
            wrappingLabelWithString: "Session bubbles, native settings, and now onboarding."
        )
        subtitle.font = .systemFont(ofSize: 12, weight: .regular)
        subtitle.textColor = NSColor.white.withAlphaComponent(0.82)
        subtitle.maximumNumberOfLines = 3

        stack.addArrangedSubview(title)
        stack.addArrangedSubview(subtitle)

        let divider = NSBox()
        divider.boxType = .separator
        divider.alphaValue = 0.35
        stack.addArrangedSubview(divider)
    }

    func addSidebarRail(to stack: NSStackView) {
        let railStack = NSStackView()
        railStack.orientation = .vertical
        railStack.alignment = .leading
        railStack.spacing = 10

        OnboardingStep.allCases.forEach { step in
            let row = OnboardingStepRowView(title: step.railLabel)
            railRows.append(row)
            railStack.addArrangedSubview(row)
        }

        stack.addArrangedSubview(railStack)

        let spacer = NSView()
        spacer.setContentHuggingPriority(.defaultLow, for: .vertical)
        stack.addArrangedSubview(spacer)
    }

    func addSidebarHint(to stack: NSStackView) {
        let hintText = "Design cues borrowed from IINA welcome and Stats settings windows."
        let hint = NSTextField(wrappingLabelWithString: hintText)
        hint.font = .systemFont(ofSize: 11, weight: .regular)
        hint.textColor = NSColor.white.withAlphaComponent(0.68)
        hint.maximumNumberOfLines = 3
        stack.addArrangedSubview(hint)
    }

    func buildMainPanel() -> NSView {
        let surface = NSVisualEffectView()
        surface.blendingMode = .behindWindow
        surface.material = .underWindowBackground
        surface.state = .active
        surface.translatesAutoresizingMaskIntoConstraints = false

        let card = makeMainCard(in: surface)
        addMainCardContents(to: card)

        return surface
    }

    func makeMainCard(in surface: NSVisualEffectView) -> NSView {
        let card = NSView()
        card.wantsLayer = true
        card.layer?.cornerRadius = 18
        card.layer?.backgroundColor = NSColor.windowBackgroundColor.withAlphaComponent(0.88).cgColor
        card.layer?.borderColor = NSColor.separatorColor.withAlphaComponent(0.28).cgColor
        card.layer?.borderWidth = 1
        card.layer?.shadowColor = NSColor.black.cgColor
        card.layer?.shadowOpacity = 0.12
        card.layer?.shadowRadius = 24
        card.layer?.shadowOffset = CGSize(width: 0, height: -4)
        card.translatesAutoresizingMaskIntoConstraints = false
        surface.addSubview(card)

        NSLayoutConstraint.activate([
            card.leadingAnchor.constraint(equalTo: surface.leadingAnchor, constant: 26),
            card.trailingAnchor.constraint(equalTo: surface.trailingAnchor, constant: -26),
            card.topAnchor.constraint(equalTo: surface.topAnchor, constant: 24),
            card.bottomAnchor.constraint(equalTo: surface.bottomAnchor, constant: -24),
            card.widthAnchor.constraint(lessThanOrEqualToConstant: 900),
            card.centerXAnchor.constraint(equalTo: surface.centerXAnchor)
        ])

        return card
    }

    func addMainCardContents(to card: NSView) {
        contentContainer.wantsLayer = true
        contentContainer.translatesAutoresizingMaskIntoConstraints = false

        let headerStack = NSStackView(views: [titleLabel, subtitleLabel])
        headerStack.orientation = .vertical
        headerStack.alignment = .leading
        headerStack.spacing = 6

        let footerButtons = NSStackView(views: [backButton, nextButton])
        footerButtons.orientation = .horizontal
        footerButtons.alignment = .centerY
        footerButtons.distribution = .fillProportionally
        footerButtons.spacing = 10

        let footer = NSStackView(views: [validationLabel, NSView(), footerButtons])
        footer.orientation = .horizontal
        footer.alignment = .centerY
        footer.spacing = 14

        let cardStack = NSStackView(views: [headerStack, contentContainer, footer])
        cardStack.orientation = .vertical
        cardStack.alignment = .leading
        cardStack.spacing = 18
        cardStack.translatesAutoresizingMaskIntoConstraints = false
        card.addSubview(cardStack)

        NSLayoutConstraint.activate([
            cardStack.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 26),
            cardStack.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -26),
            cardStack.topAnchor.constraint(equalTo: card.topAnchor, constant: 26),
            cardStack.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -26),
            contentContainer.widthAnchor.constraint(equalTo: cardStack.widthAnchor),
            contentContainer.heightAnchor.constraint(greaterThanOrEqualToConstant: 300),
            footerButtons.widthAnchor.constraint(equalToConstant: 240)
        ])
    }
}
