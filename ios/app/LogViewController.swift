// LogViewController.swift
//
// The game's log (Documents/logs/latest.log) as a screen of its own: the end
// of the file in a text view that wraps its lines to the screen's width and
// scrolls. The game's own startup log screen draws its lines unwrapped —
// wider than the phone, so the ends of the lines (often the interesting
// part) are cut off. This is the place a player can actually read what
// happened: from the launcher before a launch, and after one ended badly
// (the launcher is the phase that runs then — see main.swift).

import UIKit

final class LogViewController: UIViewController {
    /// How much of the log's end this screen loads. A run's log can grow to
    /// megabytes; the end is what a failure leaves worth reading, and the
    /// share button below sends the same text.
    private static let maxBytes = 512_000

    private let textView = UITextView()
    private var logText = ""
    private var scrolledToEnd = false

    private var gameFolder: URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = Strings.logScreenTitle
        view.backgroundColor = UIColor(red: 18 / 255, green: 18 / 255, blue: 22 / 255, alpha: 1)
        textView.translatesAutoresizingMaskIntoConstraints = false
        textView.backgroundColor = .clear
        textView.textColor = .white
        textView.font = UIFont.monospacedSystemFont(ofSize: 12, weight: .regular)
        textView.isEditable = false
        // Wrapping needs no setup: a text view wraps to its own width and
        // scrolls vertically, unlike the game's unwrapped startup screen.
        view.addSubview(textView)
        NSLayoutConstraint.activate([
            textView.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            textView.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),
            textView.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor),
            textView.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor),
        ])
        logText = loadLog()
        textView.text = logText
        navigationItem.rightBarButtonItem = UIBarButtonItem(
            title: Strings.shareButton, style: .plain, target: self, action: #selector(shareTapped)
        )
        // When this screen is the root of a presented stack (rather than
        // pushed onto the launcher's), give it a way back.
        if presentingViewController != nil, navigationController?.viewControllers.first === self {
            navigationItem.leftBarButtonItem = UIBarButtonItem(
                title: Strings.closeButton, style: .plain, target: self, action: #selector(closeTapped)
            )
        }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        // Open at the end of the log: that is where a run's last words are.
        if !scrolledToEnd {
            scrolledToEnd = true
            let end = NSRange(location: (logText as NSString).length, length: 0)
            textView.scrollRangeToVisible(end)
        }
    }

    // MARK: - Actions

    @objc private func shareTapped() {
        let sheet = UIActivityViewController(activityItems: [logText], applicationActivities: nil)
        sheet.popoverPresentationController?.barButtonItem = navigationItem.rightBarButtonItem
        present(sheet, animated: true)
    }

    @objc private func closeTapped() {
        dismiss(animated: true)
    }

    // MARK: - Files

    /// The last `maxBytes` of logs/latest.log, from the start of a line,
    /// with a note at the top when the beginning was left out.
    private func loadLog() -> String {
        let url = gameFolder.appendingPathComponent("logs/latest.log")
        guard let handle = try? FileHandle(forReadingFrom: url) else {
            return Strings.logMissing
        }
        defer { try? handle.close() }
        do {
            let length = try handle.seekToEnd()
            let count = min(length, UInt64(Self.maxBytes))
            try handle.seek(toOffset: length - count)
            let data = try handle.read(upToCount: Int(count)) ?? Data()
            var text = String(decoding: data, as: UTF8.self)
            if length > count, let newline = text.firstIndex(of: "\n") {
                text = String(text[text.index(after: newline)...])
                text = Strings.logTruncatedNote + "\n\n" + text
            }
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? Strings.logMissing : trimmed
        } catch {
            return Strings.couldNotReadFile(url.lastPathComponent, error.localizedDescription)
        }
    }
}
