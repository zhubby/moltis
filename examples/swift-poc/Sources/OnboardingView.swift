import SwiftUI

struct OnboardingView: View {
    @ObservedObject var settings: AppSettings
    let onFinish: () -> Void

    @State private var currentStep = OnboardingStep.llm

    var body: some View {
        VStack(spacing: 0) {
            NavigationSplitView {
                sidebar
            } detail: {
                detailContent
            }

            Divider()

            footerBar
        }
        .frame(minWidth: 780, minHeight: 520)
    }
}

// MARK: - Sidebar

private extension OnboardingView {
    var sidebar: some View {
        List(OnboardingStep.allCases, id: \.self, selection: $currentStep) { step in
            Label {
                Text(step.label)
            } icon: {
                Image(systemName: step.symbolName)
                    .foregroundStyle(iconColor(for: step))
            }
            .tag(step)
        }
        .navigationTitle("Setup")
        .navigationSplitViewColumnWidth(min: 180, ideal: 200)
    }

    func iconColor(for step: OnboardingStep) -> Color {
        if step.rawValue < currentStep.rawValue {
            return .green
        } else if step == currentStep {
            return .accentColor
        }
        return .secondary
    }
}

// MARK: - Detail

private extension OnboardingView {
    @ViewBuilder
    var detailContent: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                if currentStep == .summary {
                    summaryPane
                } else if let section = currentStep.settingsSection {
                    SettingsSectionContent(
                        section: section,
                        settings: settings
                    )
                }
            }
            .toggleStyle(MoltisFormToggleStyle())
            .padding(20)
            .frame(maxWidth: 600, alignment: .leading)
        }
        .scrollContentBackground(.hidden)
        .background {
            VisualEffectBackground(material: .underPageBackground)
        }
    }

    var summaryPane: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Everything looks good. You're all set.")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            MoltisSection {
                LabeledContent("Provider", value: settings.llmProvider.capitalized)
                Divider()
                LabeledContent("Model", value: settings.llmModel)
                Divider()
                LabeledContent("Voice", value: settings.voiceEnabled ? "Enabled" : "Disabled")
                Divider()
                LabeledContent("Name", value: settings.identityName.isEmpty ? "Default" : settings.identityName)
            }
        }
    }
}

// MARK: - Footer

private extension OnboardingView {
    var footerBar: some View {
        HStack {
            Text("Step \(currentStep.stepNumber) of \(OnboardingStep.totalSteps)")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            Spacer()

            if currentStep != OnboardingStep.allCases.first {
                Button("Back") {
                    goBack()
                }
                .keyboardShortcut(.cancelAction)
            }

            if currentStep == .summary {
                Button("Get Started") {
                    onFinish()
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            } else {
                Button("Next") {
                    goForward()
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 14)
    }

    func goForward() {
        let cases = OnboardingStep.allCases
        guard let index = cases.firstIndex(of: currentStep),
              cases.index(after: index) < cases.endIndex else {
            return
        }
        withAnimation(.easeInOut(duration: 0.15)) {
            currentStep = cases[cases.index(after: index)]
        }
    }

    func goBack() {
        let cases = OnboardingStep.allCases
        guard let index = cases.firstIndex(of: currentStep), index > cases.startIndex else {
            return
        }
        withAnimation(.easeInOut(duration: 0.15)) {
            currentStep = cases[cases.index(before: index)]
        }
    }
}
