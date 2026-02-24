import AppKit
import SwiftUI

// MARK: - VisualEffectBackground

struct VisualEffectBackground: NSViewRepresentable {
    let material: NSVisualEffectView.Material
    let blendingMode: NSVisualEffectView.BlendingMode

    init(
        material: NSVisualEffectView.Material = .underPageBackground,
        blendingMode: NSVisualEffectView.BlendingMode = .behindWindow
    ) {
        self.material = material
        self.blendingMode = blendingMode
    }

    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.material = material
        view.blendingMode = blendingMode
        view.state = .followsWindowActiveState
        return view
    }

    func updateNSView(_ nsView: NSVisualEffectView, context: Context) {
        nsView.material = material
        nsView.blendingMode = blendingMode
    }
}

// MARK: - MoltisGroupBox (matches Ice's IceGroupBox)

struct MoltisGroupBox<Content: View>: View {
    private let padding: CGFloat
    private let content: Content

    private var backgroundShape: some InsettableShape {
        RoundedRectangle(cornerRadius: 6, style: .circular)
    }

    init(padding: CGFloat = 10, @ViewBuilder content: () -> Content) {
        self.padding = padding
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading) {
            content
        }
        .padding(padding)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            backgroundShape
                .fill(.quinary)
                .overlay {
                    backgroundShape
                        .strokeBorder(.quaternary)
                }
        }
    }
}

// MARK: - MoltisSection (titled group box with dividers)

struct MoltisSection<Content: View>: View {
    private let title: String?
    private let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.title = nil
        self.content = content()
    }

    init(_ title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading) {
            if let title {
                Text(title)
                    .font(.headline)
            }
            MoltisGroupBox {
                content
            }
        }
    }
}

// MARK: - MoltisFormToggleStyle (label left, switch right — matches Ice)

struct MoltisFormToggleStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        LabeledContent {
            Toggle(isOn: configuration.$isOn) {
                configuration.label
            }
            .labelsHidden()
            .toggleStyle(.switch)
            .controlSize(.mini)
        } label: {
            configuration.label
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

// MARK: - Annotation modifier (matches Ice's AnnotationView)

extension View {
    func annotation(_ text: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            self
            Text(text)
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

// MARK: - MoltisEditorField

struct MoltisEditorField: View {
    let title: String
    @Binding var text: String
    let minHeight: CGFloat

    init(title: String, text: Binding<String>, minHeight: CGFloat = 180) {
        self.title = title
        _text = text
        self.minHeight = minHeight
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.subheadline)
                .foregroundStyle(.secondary)

            TextEditor(text: $text)
                .font(.system(.body, design: .monospaced))
                .scrollContentBackground(.hidden)
                .padding(8)
                .frame(minHeight: minHeight)
                .background(Color(nsColor: .controlBackgroundColor))
                .clipShape(RoundedRectangle(cornerRadius: 6, style: .circular))
                .overlay {
                    RoundedRectangle(cornerRadius: 6, style: .circular)
                        .strokeBorder(.quaternary)
                }
        }
    }
}
