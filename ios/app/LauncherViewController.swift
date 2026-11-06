// LauncherViewController.swift
//
// The app's front door, and the readable error screen: it says whether the
// game's resources are on the phone and where to put them, imports them from a
// zip or a folder the player picked in the Files app, and when a run ended badly
// shows why — the message the game left in last-error.txt, or, if the process
// died without one (a crash in the graphics driver), the end of the log.
//
// This mirrors the Android launcher (LauncherActivity.java): same screens, same
// files (`last-error.txt`, `running.marker`, `logs/latest.log`), same import
// contract. One difference is forced by iOS: after a successful import the
// player relaunches the app (closes it and opens it again) instead of tapping
// Start, because winit cannot start its event loop in a process where
// `UIApplicationMain` has already run — see main.swift and ios/README.md.
//
// A separate log screen (LogViewController) shows the end of the log wrapped
// to the screen's width: the game's own startup log screen does not wrap, so
// its lines run off the phone's edge and the ends of them cannot be read.

import UIKit
import UniformTypeIdentifiers

final class LauncherViewController: UIViewController, UIDocumentPickerDelegate {
    /// `horizon_ios_resources_state` codes (Horizon-Bridging-Header.h): 0 the
    /// resources are missing, 1 they are in resources/. These two name where
    /// else the game accepts them from.
    private let resourcesInPrepared: Int32 = 2
    private let resourcesFromArgs: Int32 = 3

    private let importer = ResourceImporter()

    private let scrollView = UIScrollView()
    private let column = UIStackView()
    private var importStatusView: UILabel?
    private var isImporting = false
    private var importStatus = ""
    private var importResult: String?

    private var gameFolder: URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = Strings.appTitle
        view.backgroundColor = UIColor(red: 18 / 255, green: 18 / 255, blue: 22 / 255, alpha: 1)
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(scrollView)
        NSLayoutConstraint.activate([
            scrollView.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            scrollView.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),
            scrollView.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor),
        ])
        column.axis = .vertical
        column.spacing = 12
        column.translatesAutoresizingMaskIntoConstraints = false
        column.isLayoutMarginsRelativeArrangement = true
        column.layoutMargins = UIEdgeInsets(top: 28, left: 20, bottom: 20, right: 20)
        scrollView.addSubview(column)
        NSLayoutConstraint.activate([
            column.topAnchor.constraint(equalTo: scrollView.topAnchor),
            column.bottomAnchor.constraint(equalTo: scrollView.bottomAnchor),
            column.leadingAnchor.constraint(equalTo: scrollView.leadingAnchor),
            column.trailingAnchor.constraint(equalTo: scrollView.trailingAnchor),
            column.widthAnchor.constraint(equalTo: scrollView.widthAnchor),
        ])
        show()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        show()
    }

    // MARK: - Layout

    private func makeText(_ body: String, size: CGFloat, color: UIColor, mono: Bool = false) -> UILabel {
        let label = UILabel()
        label.text = body
        label.font = mono
            ? UIFont.monospacedSystemFont(ofSize: size, weight: .regular)
            : UIFont.systemFont(ofSize: size)
        label.textColor = color
        label.numberOfLines = 0
        return label
    }

    private func makeButton(_ title: String, action: Selector) -> UIButton {
        var config = UIButton.Configuration.filled()
        config.title = title
        config.baseBackgroundColor = UIColor(red: 0.25, green: 0.25, blue: 0.32, alpha: 1)
        config.baseForegroundColor = .white
        config.contentInsets = NSDirectionalEdgeInsets(top: 12, leading: 16, bottom: 12, trailing: 16)
        let button = UIButton(configuration: config, primaryAction: nil)
        button.addTarget(self, action: action, for: .touchUpInside)
        return button
    }

    private func show() {
        column.arrangedSubviews.forEach { $0.removeFromSuperview() }
        importStatusView = nil
        let folder = gameFolder
        let errorURL = folder.appendingPathComponent("last-error.txt")
        let markerURL = folder.appendingPathComponent("running.marker")
        let manifestURL = folder.appendingPathComponent("resources/horizon-resources.json")
        // The game's own verdict on where the resources are (the Rust shell's
        // `resources_state`, over the bridge): this screen's old check knew
        // only resources/, so it called the resources missing on phones where
        // the game would have found them in the prepared folder or at a
        // horizon-args.txt override.
        let resourcesState = folder.path.withCString { horizon_ios_resources_state($0) }
        let ready = resourcesState != 0
        let white = UIColor.white
        let grey = UIColor(red: 170 / 255, green: 170 / 255, blue: 180 / 255, alpha: 1)
        let red = UIColor(red: 1, green: 140 / 255, blue: 120 / 255, alpha: 1)

        let title = makeText(Strings.appTitle, size: 30, color: white)
        column.addArrangedSubview(title)
        let device = "\(UIDevice.current.model) \(UIDevice.current.systemVersion)"
        column.addArrangedSubview(makeText(Strings.subtitle(device: device), size: 14, color: grey))

        if FileManager.default.fileExists(atPath: errorURL.path) {
            column.addArrangedSubview(makeText(Strings.lastRunErrorHeading, size: 16, color: red))
            column.addArrangedSubview(makeText(readTail(errorURL, maxBytes: 6000), size: 13, color: white, mono: true))
        } else if FileManager.default.fileExists(atPath: markerURL.path) {
            column.addArrangedSubview(makeText(Strings.lastRunUnknownHeading, size: 16, color: red))
            column.addArrangedSubview(makeText(
                readTail(folder.appendingPathComponent("logs/latest.log"), maxBytes: 6000),
                size: 12, color: white, mono: true
            ))
        }

        if isImporting {
            column.addArrangedSubview(makeText(Strings.importingHeading, size: 15, color: white))
            let status = makeText(importStatus, size: 13, color: UIColor(red: 140 / 255, green: 200 / 255, blue: 1, alpha: 1), mono: true)
            importStatusView = status
            column.addArrangedSubview(status)
            return
        }
        if let result = importResult {
            column.addArrangedSubview(makeText(result, size: 14, color: UIColor(red: 1, green: 200 / 255, blue: 120 / 255, alpha: 1)))
        }
        if !ready {
            column.addArrangedSubview(makeText(Strings.resourcesMissingBody, size: 15, color: white))
            column.addArrangedSubview(makeText(Strings.importDestination(manifestURL.deletingLastPathComponent().path), size: 13, color: grey))
        } else if resourcesState == resourcesInPrepared {
            column.addArrangedSubview(makeText(Strings.resourcesFoundPrepared, size: 13, color: grey))
        } else if resourcesState == resourcesFromArgs {
            column.addArrangedSubview(makeText(Strings.resourcesFoundArgs, size: 13, color: grey))
        } else {
            column.addArrangedSubview(makeText(
                Strings.resourcesFound(manifestURL.deletingLastPathComponent().path), size: 13, color: grey
            ))
        }

        column.addArrangedSubview(makeButton(ready ? Strings.importFolderReplace : Strings.importFolderReady, action: #selector(pickFolder)))
        column.addArrangedSubview(makeButton(ready ? Strings.importZipReplace : Strings.importZipReady, action: #selector(pickZip)))
        column.addArrangedSubview(makeButton(ready ? Strings.startGame : Strings.startAnyway, action: #selector(startGameTapped)))
        column.addArrangedSubview(makeButton(Strings.viewLog, action: #selector(viewLogTapped)))
        column.addArrangedSubview(makeButton(Strings.shareLog, action: #selector(shareLogTapped)))
        if FileManager.default.fileExists(atPath: errorURL.path)
            || FileManager.default.fileExists(atPath: markerURL.path) {
            column.addArrangedSubview(makeButton(Strings.dismiss, action: #selector(dismissTapped)))
        }
        column.addArrangedSubview(makeText(Strings.gameFolderLine(folder.path), size: 12, color: grey, mono: true))
    }

    // MARK: - Actions

    @objc private func pickFolder() {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.folder])
        picker.delegate = self
        picker.title = Strings.pickFolderTitle
        present(picker, animated: true)
    }

    @objc private func pickZip() {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.zip])
        picker.delegate = self
        picker.title = Strings.pickZipTitle
        present(picker, animated: true)
    }

    @objc private func startGameTapped() {
        // There is no in-process start: the game runs on the next launch, once
        // this process (which already called `UIApplicationMain`) is gone. The
        // player closes the app and opens it again; on that launch the resources
        // are there and `main.swift` hands the process to the game.
        importResult = Strings.startHint
        show()
    }

    @objc private func viewLogTapped() {
        let log = LogViewController()
        if let navigation = navigationController {
            navigation.pushViewController(log, animated: true)
        } else {
            // No stack to push onto (the delegate roots the launcher in one,
            // so this is a fallback): present the log screen with its own.
            present(UINavigationController(rootViewController: log), animated: true)
        }
    }

    @objc private func shareLogTapped() {
        let log = readTail(gameFolder.appendingPathComponent("logs/latest.log"), maxBytes: 60000)
        let sheet = UIActivityViewController(activityItems: [log], applicationActivities: nil)
        present(sheet, animated: true)
    }

    @objc private func dismissTapped() {
        let folder = gameFolder
        try? FileManager.default.removeItem(at: folder.appendingPathComponent("last-error.txt"))
        try? FileManager.default.removeItem(at: folder.appendingPathComponent("running.marker"))
        show()
    }

    // MARK: - UIDocumentPickerDelegate

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let url = urls.first else { return }
        isImporting = true
        importResult = nil
        importStatus = Strings.progressStarting
        show()
        // The zip flow is picked by extension: the folder picker returns a
        // directory, the zip picker a `.zip` file.
        let isZip = url.pathExtension.lowercased() == "zip"
        importer.importResources(from: url, zip: isZip, progress: { [weak self] status in
            DispatchQueue.main.async {
                self?.importStatus = status
                self?.importStatusView?.text = status
            }
        }, completion: { [weak self] result in
            DispatchQueue.main.async {
                guard let self else { return }
                self.isImporting = false
                switch result {
                case .success:
                    self.importResult = Strings.importedBody
                case .failure(let error):
                    self.importResult = Strings.importFailed(error.localizedDescription)
                }
                self.show()
            }
        })
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        // Nothing picked: stay on the launcher.
    }

    // MARK: - Files

    /// The last `maxBytes` bytes of a text file, from the start of a line.
    private func readTail(_ url: URL, maxBytes: Int) -> String {
        guard let handle = try? FileHandle(forReadingFrom: url) else {
            return Strings.missingFileNote
        }
        defer { try? handle.close() }
        do {
            let length = try handle.seekToEnd()
            let count = min(length, UInt64(maxBytes))
            try handle.seek(toOffset: length - count)
            let data = try handle.read(upToCount: Int(count)) ?? Data()
            var text = String(decoding: data, as: UTF8.self)
            if length > count, let newline = text.firstIndex(of: "\n") {
                text = String(text[text.index(after: newline)...])
            }
            return text.trimmingCharacters(in: .whitespacesAndNewlines)
        } catch {
            return Strings.couldNotReadFile(url.lastPathComponent, error.localizedDescription)
        }
    }
}
