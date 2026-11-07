// LogExport.swift
//
// Puts the game's log where the Files app shows it. The game writes into
// subfolders of the game folder (the app's Documents directory):
// `logs/latest.log` for the log, `crash-reports/` for crash reports. Those
// are the game's own layout — the desktop game and its tools read them
// there — but in the Files app a subfolder is easy to miss when all a
// player wants is "the log". So this copies the log, and the newest crash
// report under its own name, to the top of Documents, where Files lists
// them by name and a share sheet can pick them up.
//
// `main.swift` calls this on every launch before the game phase runs — a
// run that ended in a hard crash never returned to Swift, so its log is
// exported on the next open, before the new run's logging rotates the old
// `latest.log` away — and again when the game phase returns, to export the
// run that just ended. Best effort throughout: a missing or empty source
// (the first launch has no log yet) is skipped silently and leaves any
// earlier export untouched.

import Foundation

func exportGameLogToDocuments() {
    let files = FileManager.default
    guard let folder = files.urls(for: .documentDirectory, in: .userDomainMask).first else {
        return
    }

    let log = folder.appendingPathComponent("logs/latest.log")
    if fileSize(log) > 0 {
        let exported = folder.appendingPathComponent("latest.log")
        try? files.removeItem(at: exported)
        try? files.copyItem(at: log, to: exported)
    }

    if let newest = newestFile(in: folder.appendingPathComponent("crash-reports")) {
        let exported = folder.appendingPathComponent(newest.lastPathComponent)
        // A report's file name carries its timestamp, so one already at the
        // top under that name is the same report.
        if !files.fileExists(atPath: exported.path) {
            try? files.copyItem(at: newest, to: exported)
        }
    }
}

private func fileSize(_ url: URL) -> UInt64 {
    let values = try? url.resourceValues(forKeys: [.fileSizeKey])
    return UInt64(values?.fileSize ?? 0)
}

/// The most recently modified file directly inside `folder`, or nil when
/// the folder is missing or empty.
private func newestFile(in folder: URL) -> URL? {
    func modified(_ url: URL) -> Date {
        let values = try? url.resourceValues(forKeys: [.contentModificationDateKey])
        return values?.contentModificationDate ?? .distantPast
    }
    guard let entries = try? FileManager.default.contentsOfDirectory(
        at: folder, includingPropertiesForKeys: [.contentModificationDateKey]
    ) else {
        return nil
    }
    return entries.max { modified($0) < modified($1) }
}
