import AppKit

final class OnboardingGradientView: NSView {
    private let gradientLayer = CAGradientLayer()

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        gradientLayer.colors = [
            NSColor(calibratedRed: 0.08, green: 0.24, blue: 0.44, alpha: 1).cgColor,
            NSColor(calibratedRed: 0.10, green: 0.40, blue: 0.65, alpha: 1).cgColor
        ]
        gradientLayer.startPoint = CGPoint(x: 0, y: 1)
        gradientLayer.endPoint = CGPoint(x: 1, y: 0)
        layer?.addSublayer(gradientLayer)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layout() {
        super.layout()
        gradientLayer.frame = bounds
    }
}

final class OnboardingStepRowView: NSView {
    private let dotView = NSView()
    private let textField = NSTextField(labelWithString: "")

    init(title: String) {
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false

        dotView.wantsLayer = true
        dotView.layer?.cornerRadius = 5
        dotView.translatesAutoresizingMaskIntoConstraints = false

        textField.stringValue = title
        textField.font = .systemFont(ofSize: 13, weight: .medium)
        textField.translatesAutoresizingMaskIntoConstraints = false

        addSubview(dotView)
        addSubview(textField)

        NSLayoutConstraint.activate([
            dotView.leadingAnchor.constraint(equalTo: leadingAnchor),
            dotView.centerYAnchor.constraint(equalTo: centerYAnchor),
            dotView.widthAnchor.constraint(equalToConstant: 10),
            dotView.heightAnchor.constraint(equalToConstant: 10),
            textField.leadingAnchor.constraint(equalTo: dotView.trailingAnchor, constant: 9),
            textField.trailingAnchor.constraint(equalTo: trailingAnchor),
            textField.topAnchor.constraint(equalTo: topAnchor, constant: 2),
            textField.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -2)
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func apply(state: RailStepState) {
        switch state {
        case .upcoming:
            dotView.layer?.backgroundColor = NSColor.white.withAlphaComponent(0.35).cgColor
            textField.textColor = NSColor.white.withAlphaComponent(0.72)
        case .current:
            dotView.layer?.backgroundColor = NSColor.white.cgColor
            textField.textColor = .white
        case .done:
            let doneColor = NSColor(calibratedRed: 0.66, green: 0.93, blue: 0.76, alpha: 1)
            dotView.layer?.backgroundColor = doneColor.cgColor
            textField.textColor = NSColor.white.withAlphaComponent(0.9)
        }
    }
}
