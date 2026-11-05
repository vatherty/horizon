// ResourceImporter.swift
//
// Copies the prepared resources into the game folder from a zip or a folder
// the player picked in the Files app. The contract is the Android one
// (LauncherActivity.java, ANDROID.md "Resources"):
//
//   - the prepared folder is the one that holds `horizon-resources.json`;
//   - it lands at `<Documents>/resources/` (so `resources/horizon-resources.json`
//     exists), replacing whatever was there;
//   - a zip may wrap the folder in one extra level (`horizon-resources.zip`
//     usually holds the files at its root; a zip of the folder itself holds
//     them one level down), so the manifest is searched for, a few levels deep.
//
// The work runs on a background thread; progress lines go back on the caller's
// queue through `progress`. The import writes to a staging folder first and
// only moves it into place once the manifest is found, so a failed import
// never leaves a half-copied `resources/` behind.

import Foundation

final class ResourceImporter {
    private static let manifest = "horizon-resources.json"

    private var gameFolder: URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
    }

    /// Imports `source` (a zip when `zip` is true, else a folder) into the game
    /// folder. Calls `completion` on a background thread.
    func importResources(
        from source: URL,
        zip: Bool,
        progress: @escaping (String) -> Void,
        completion: @escaping (Result<Void, Error>) -> Void
    ) {
        DispatchQueue.global(qos: .userInitiated).async {
            do {
                try self.runImport(from: source, zip: zip, progress: progress)
                completion(.success(()))
            } catch {
                completion(.failure(error))
            }
        }
    }

    private func runImport(from source: URL, zip: Bool, progress: @escaping (String) -> Void) throws {
        let folder = gameFolder
        let staging = folder.appendingPathComponent("resources.importing")
        let files = FileManager.default
        try? files.removeItem(at: staging)
        do {
            try files.createDirectory(at: staging, withIntermediateDirectories: true)
        } catch {
            throw ImportError.message(Strings.couldNotCreate(staging.path))
        }
        // The picked file lives outside the sandbox: hold access for the whole
        // import, and release it when this scope ends.
        let scoped = source.startAccessingSecurityScopedResource()
        defer {
            if scoped { source.stopAccessingSecurityScopedResource() }
            try? files.removeItem(at: staging)
        }
        var root: URL?
        if zip {
            progress(Strings.progressUnpacking)
            var lastReported: Int64 = 0
            try ZipExtractor.extract(source, into: staging, progress: { written in
                // A label update per megabyte would flood the main thread; report
                // every 8 MB instead.
                if written - lastReported >= 8 * 1024 * 1024 {
                    lastReported = written
                    progress(Strings.progressUnpacked(Int(written / (1024 * 1024))))
                }
            })
            root = findManifestDir(staging, depth: 4)
        } else {
            progress(Strings.progressLookingForManifest)
            guard let manifestDir = findManifestDir(source, depth: 3) else {
                throw ImportError.message(Strings.noManifestInFolder)
            }
            progress(Strings.progressCopying)
            try copyTree(manifestDir, into: staging, progress: { written, name in
                progress(Strings.progressCopied(Int(written / (1024 * 1024)), name))
            })
            root = staging
        }
        guard let root, files.fileExists(atPath: root.appendingPathComponent(Self.manifest).path) else {
            throw ImportError.message(Strings.noManifestInZip)
        }
        progress(Strings.progressPuttingInPlace)
        let target = folder.appendingPathComponent("resources")
        try? files.removeItem(at: target)
        do {
            try files.moveItem(at: root, to: target)
        } catch {
            throw ImportError.message(Strings.couldNotMoveInto(target.path))
        }
        // The prepared folder's natural place, as a link to `resources/` — the
        // same bridge the Rust entry keeps (ios/src/lib.rs
        // `ensure_prepared_symlink`); the launcher keeps it too, so the game
        // finds the resources on the very next launch.
        try linkPreparedDir(folder: folder)
        // The staging folder is gone (moved) when the root was the staging
        // folder itself; the defer above removes whatever is left.
        if root != staging {
            try? files.removeItem(at: staging)
        }
    }

    /// The folder (at most `depth` levels below `dir`) that holds the manifest.
    private func findManifestDir(_ dir: URL, depth: Int) -> URL? {
        let files = FileManager.default
        if files.fileExists(atPath: dir.appendingPathComponent(Self.manifest).path) {
            return dir
        }
        guard depth > 0,
              let children = try? files.contentsOfDirectory(
                at: dir, includingPropertiesForKeys: [.isDirectoryKey]) else {
            return nil
        }
        for child in children {
            var isDir: ObjCBool = false
            guard files.fileExists(atPath: child.path, isDirectory: &isDir), isDir.boolValue else {
                continue
            }
            if let found = findManifestDir(child, depth: depth - 1) {
                return found
            }
        }
        return nil
    }

    /// A recursive copy of a picked folder, with the same traversal guard as the
    /// zip extractor: nothing outside the picked folder is followed.
    private func copyTree(
        _ source: URL,
        into destination: URL,
        progress: (Int64, String) -> Void
    ) throws {
        let files = FileManager.default
        var written: Int64 = 0
        var stack: [(URL, URL)] = [(source, destination)]
        while let (from, to) = stack.popLast() {
            var isDir: ObjCBool = false
            guard files.fileExists(atPath: from.path, isDirectory: &isDir) else {
                throw ImportError.message(Strings.couldNotOpenFile(from.lastPathComponent))
            }
            if isDir.boolValue {
                try files.createDirectory(at: to, withIntermediateDirectories: true)
                let children = try files.contentsOfDirectory(at: from, includingPropertiesForKeys: nil)
                for child in children {
                    stack.append((child, to.appendingPathComponent(child.lastPathComponent)))
                }
            } else {
                do {
                    try files.copyItem(at: from, to: to)
                } catch {
                    throw ImportError.message(Strings.couldNotOpenFile(from.lastPathComponent))
                }
                if let size = (try? files.attributesOfItem(atPath: from.path)[.size] as? NSNumber)?.int64Value {
                    written += size
                }
                progress(written, from.lastPathComponent)
            }
        }
    }

    /// `.local/horizon-client/26.3` as a link to `resources/` (see
    /// `ensure_prepared_symlink` in ios/src/lib.rs). A real prepared folder
    /// there wins and is left alone.
    private func linkPreparedDir(folder: URL) throws {
        let files = FileManager.default
        let prepared = folder
            .appendingPathComponent(".local")
            .appendingPathComponent("horizon-client")
            .appendingPathComponent("26.3")
        if files.fileExists(atPath: prepared.path) {
            return
        }
        // `fileExists` follows links, so check the link itself too: a stale link
        // is replaced, a good one is left alone.
        if (try? files.destinationOfSymbolicLink(atPath: prepared.path)) != nil {
            return
        }
        try files.createDirectory(
            at: prepared.deletingLastPathComponent(), withIntermediateDirectories: true
        )
        do {
            try files.createSymbolicLink(atPath: prepared.path, withDestinationPath: "../../resources")
        } catch {
            // The link is only a bridge for the unmodified game; the import
            // itself succeeded without it, so a failure here is not fatal.
        }
    }

    enum ImportError: LocalizedError {
        case message(String)
        var errorDescription: String? {
            if case .message(let text) = self { return text }
            return nil
        }
    }
}
