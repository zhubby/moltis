import SwiftUI

struct OnboardingHostView: NSViewControllerRepresentable {
    @ObservedObject var settings: AppSettings
    let onFinish: () -> Void

    func makeNSViewController(context: Context) -> OnboardingViewController {
        OnboardingViewController(settings: settings, onFinish: onFinish)
    }

    func updateNSViewController(
        _ viewController: OnboardingViewController,
        context: Context
    ) {
        viewController.syncSettings()
    }
}
