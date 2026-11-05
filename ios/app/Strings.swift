// Strings.swift
//
// Every user-facing string in the Swift shell lives here, in one place, so
// none is scattered through the view code (upstream rule: no hardcoded
// user-facing strings — AGENTS.md). There is no `tr()` in this thin Swift shell
// yet; when a strings catalog is added, these move there unchanged, keyed by
// the comment above each group.

import Foundation

/// All user-facing text in the iOS shell, grouped by screen.
enum Strings {
    // MARK: Launcher

    static let appTitle = "Horizon"
    static func subtitle(device: String) -> String {
        "Minecraft Java Edition 26.3, rewritten in Rust. \(device)."
    }
    static let lastRunErrorHeading = "The last run ended with an error:"
    static let lastRunUnknownHeading =
        "The last run stopped without saying why (the game or the graphics driver crashed). " +
        "The end of the log:"
    static let missingFileNote = "(does not exist)"
    static func couldNotReadFile(_ name: String, _ error: String) -> String {
        "(could not read \(name): \(error))"
    }

    static let resourcesMissingBody =
        "The game's resources are not on this phone yet.\n\n" +
        "Horizon does not include Minecraft's files. On a computer, prepare them " +
        "from your own Minecraft Java Edition install (the Horizon launcher's " +
        "Prepare button), zip the prepared folder (the one holding " +
        "horizon-resources.json), and send the zip to this phone, for instance to " +
        "its Download folder — then tap one of the import buttons below."

    static func resourcesFound(_ path: String) -> String {
        "Resources found in \(path)"
    }
    static func importDestination(_ path: String) -> String {
        "Or put them there yourself:\n\(path)"
    }

    static let importZipReady = "Import resources from a zip"
    static let importZipReplace = "Replace the resources: import a zip"
    static let importFolderReady = "Import resources from a folder"
    static let importFolderReplace = "Replace the resources: import a folder"
    static let startGame = "Start Horizon"
    static let startAnyway = "Start anyway (shows the error)"
    static let shareLog = "Share the log"
    static let dismiss = "Dismiss"
    static let importingHeading = "Importing the resources. This takes a few minutes; keep the app open."
    static let importedBody = "Imported. Close Horizon and open it again to play."
    static let importedTitle = "Imported"
    static func importFailed(_ error: String) -> String {
        "The import failed: \(error)"
    }
    static func gameFolderLine(_ folder: String) -> String {
        "Game folder: \(folder)\nLog: logs/latest.log in it"
    }
    static let pickZipTitle = "Pick the prepared-resources zip"
    static let pickFolderTitle = "Pick the prepared folder"

    // MARK: Import progress

    static let progressStarting = "Starting"
    static let progressUnpacking = "Unpacking the zip"
    static let progressLookingForManifest = "Looking for horizon-resources.json"
    static let progressCopying = "Copying the folder"
    static let progressPuttingInPlace = "Putting the files in place"
    static func progressUnpacked(_ megabytes: Int) -> String {
        "\(megabytes) MB unpacked"
    }
    static func progressCopied(_ megabytes: Int, _ name: String) -> String {
        "\(megabytes) MB copied\n\(name)"
    }
    static func couldNotOpenFile(_ name: String) -> String {
        "could not open \(name)"
    }
    static func couldNotCreate(_ path: String) -> String {
        "could not create \(path)"
    }
    static func couldNotMoveInto(_ path: String) -> String {
        "could not move the files into \(path)"
    }
    static let noManifestInZip =
        "no horizon-resources.json in what was copied. Is this the prepared folder?"
    static let noManifestInFolder =
        "no horizon-resources.json in that folder. Pick the prepared folder itself " +
        "(the one that holds horizon-resources.json)."
    static let zipEntryNotSupported =
        "the zip uses a compression or feature this importer does not support " +
        "(encrypted entries and zip64 entries over 4 GB are not supported)"
    static let zipUnreadable = "the zip could not be read"
    static let zipEntryUnsafe = "the zip holds an entry that would escape the game folder"
}
