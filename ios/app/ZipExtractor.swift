// ZipExtractor.swift
//
// A minimal streaming zip reader for the prepared-resources zip: stored and
// deflated entries, zip64 end-of-central-directory records, no encryption.
//
// Why hand-rolled: iOS ships no unzip API, and this shell takes no third-party
// dependency. The workhorse is zlib itself (`<zlib.h>`, via the bridging
// header, linked with `-lz`): zip's method 8 is a raw DEFLATE stream, so the
// inflate call uses a negative window (`-MAX_WBITS`, no zlib header).
//
// Limits, by design: no encrypted entries, no multi-disk archives, no
// compression method other than stored (0) and deflate (8), and no per-entry
// zip64 sizes above 4 GB (the prepared zip is ~1–2 GB total). Anything outside
// that fails with a readable message, and the folder import in
// ResourceImporter stays as the fallback. Entries that would escape the
// destination (absolute paths, `..`) are rejected, as in the Android
// importer's canonical-path check.

import Foundation

enum ZipExtractor {
    /// Unpacks `zipURL` into `directory` (created when missing). `progress`
    /// reports the total uncompressed bytes written, on the caller's thread.
    static func extract(
        _ zipURL: URL,
        into directory: URL,
        progress: (Int64) -> Void
    ) throws {
        let files = FileManager.default
        try files.createDirectory(at: directory, withIntermediateDirectories: true)
        guard let input = try? FileHandle(forReadingFrom: zipURL) else {
            throw ZipError.unreadable
        }
        defer { try? input.close() }
        let size = (try? files.attributesOfItem(atPath: zipURL.path)[.size] as? NSNumber)?.uint64Value ?? 0
        let central = try readCentralDirectory(input, fileSize: size)
        var written: Int64 = 0
        for entry in central {
            guard let relative = safeRelativePath(entry.name) else {
                throw ZipError.unsafeEntry
            }
            if entry.name.hasSuffix("/") || entry.name.hasPrefix("__MACOSX/") {
                continue
            }
            let outURL = directory.appendingPathComponent(relative)
            guard let parent = outURL.deletingLastPathComponent() as URL?,
                  (try? files.createDirectory(at: parent, withIntermediateDirectories: true)) != nil else {
                throw ZipError.unreadable
            }
            guard files.createFile(atPath: outURL.path, contents: nil),
                  let output = try? FileHandle(forWritingTo: outURL) else {
                throw ZipError.unreadable
            }
            defer { try? output.close() }
            switch entry.method {
            case 0:
                try copyStored(input, offset: entry.dataOffset, size: entry.compressedSize, to: output, progress: &written, report: progress)
            case 8:
                try inflateEntry(input, offset: entry.dataOffset, size: entry.compressedSize, to: output, progress: &written, report: progress)
            default:
                throw ZipError.unsupported
            }
        }
    }

    // MARK: - Central directory

    private struct Entry {
        let name: String
        let method: UInt16
        let compressedSize: UInt64
        let localHeaderOffset: UInt64
        var dataOffset: UInt64 = 0
    }

    /// Reads the central directory and resolves each entry's data offset from
    /// its local header. `fileSize` is the zip's length in bytes.
    private static func readCentralDirectory(_ input: FileHandle, fileSize: UInt64) throws -> [Entry] {
        // The end-of-central-directory record is in the last 64 KiB + 22 bytes
        // (its comment is at most 64 KiB long).
        let tailLength = min(fileSize, 65557)
        try input.seek(toOffset: fileSize - tailLength)
        guard let tail = try input.read(upToCount: Int(tailLength)), tail.count == tailLength else {
            throw ZipError.unreadable
        }
        // The end-of-central-directory record is in the last 64 KiB + 22 bytes
        // (its comment is at most 64 KiB long): scan backwards for its signature.
        var eocd: Int?
        var i = tail.count - 22
        while i >= 0 {
            if tail[i] == 0x50 && tail[i + 1] == 0x4B && tail[i + 2] == 0x05 && tail[i + 3] == 0x06 {
                eocd = i
                break
            }
            i -= 1
        }
        guard let eocd else { throw ZipError.unreadable }
        var totalEntries = UInt64(tail.u16(at: eocd + 10))
        var cdSize = UInt64(tail.u32(at: eocd + 12))
        var cdOffset = UInt64(tail.u32(at: eocd + 16))
        if totalEntries == 0xFFFF || cdSize == 0xFFFF_FFFF || cdOffset == 0xFFFF_FFFF {
            // Zip64 end-of-central-directory locator: 20 bytes just before the
            // EOCD, pointing at the zip64 EOCD record.
            let locatorAt = eocd - 20
            guard locatorAt >= 0,
                  tail.u32(at: locatorAt) == 0x0706_4B50 else {
                throw ZipError.unsupported
            }
            let zip64Offset = tail.u64(at: locatorAt + 8)
            try input.seek(toOffset: zip64Offset)
            guard let record = try input.read(upToCount: 56), record.count == 56,
                  record.u32(at: 0) == 0x0606_4B50 else {
                throw ZipError.unsupported
            }
            totalEntries = record.u64(at: 32)
            cdSize = record.u64(at: 40)
            cdOffset = record.u64(at: 48)
        }
        try input.seek(toOffset: cdOffset)
        guard let cd = try input.read(upToCount: Int(cdSize)), cd.count == cdSize else {
            throw ZipError.unreadable
        }
        var entries: [Entry] = []
        var cursor = 0
        while cursor + 46 <= cd.count, entries.count < totalEntries {
            guard cd.u32(at: cursor) == 0x0201_4B50 else { break }
            let flags = cd.u16(at: cursor + 8)
            let method = cd.u16(at: cursor + 10)
            var compressedSize = UInt64(cd.u32(at: cursor + 20))
            var uncompressedSize = UInt64(cd.u32(at: cursor + 24))
            let nameLength = Int(cd.u16(at: cursor + 28))
            let extraLength = Int(cd.u16(at: cursor + 30))
            let commentLength = Int(cd.u16(at: cursor + 32))
            var localHeaderOffset = UInt64(cd.u32(at: cursor + 42))
            guard cursor + 46 + nameLength + extraLength + commentLength <= cd.count else {
                throw ZipError.unreadable
            }
            let nameData = cd.subdata(in: (cursor + 46)..<(cursor + 46 + nameLength))
            let extra = cd.subdata(in: (cursor + 46 + nameLength)..<(cursor + 46 + nameLength + extraLength))
            // Zip64 extra field (0x0001): 8-byte values in order — uncompressed
            // size, compressed size, local header offset, disk start — for each
            // field that was 0xFFFF_FFFF in the central directory.
            if compressedSize == 0xFFFF_FFFF || uncompressedSize == 0xFFFF_FFFF || localHeaderOffset == 0xFFFF_FFFF {
                var extraCursor = 0
                while extraCursor + 4 <= extra.count {
                    let tag = extra.u16(at: extraCursor)
                    let size = Int(extra.u16(at: extraCursor + 2))
                    guard extraCursor + 4 + size <= extra.count else { break }
                    if tag == 0x0001 {
                        var field = extraCursor + 4
                        if uncompressedSize == 0xFFFF_FFFF, field + 8 <= extra.count {
                            uncompressedSize = extra.u64(at: field); field += 8
                        }
                        if compressedSize == 0xFFFF_FFFF, field + 8 <= extra.count {
                            compressedSize = extra.u64(at: field); field += 8
                        }
                        if localHeaderOffset == 0xFFFF_FFFF, field + 8 <= extra.count {
                            localHeaderOffset = extra.u64(at: field); field += 8
                        }
                    }
                    extraCursor += 4 + size
                }
            }
            _ = uncompressedSize
            if flags & 0x1 != 0 {
                // Encrypted entry: not supported, and failing here (before any
                // file is written for it) keeps the staging folder clean.
                throw ZipError.unsupported
            }
            guard let name = String(data: nameData, encoding: .utf8) ?? String(data: nameData, encoding: .isoLatin1) else {
                throw ZipError.unreadable
            }
            var entry = Entry(name: name, method: method, compressedSize: compressedSize, localHeaderOffset: localHeaderOffset)
            // The local header's own name/extra lengths say where the data starts.
            try input.seek(toOffset: localHeaderOffset)
            guard let local = try input.read(upToCount: 30), local.count == 30,
                  local.u32(at: 0) == 0x0403_4B50 else {
                throw ZipError.unreadable
            }
            entry.dataOffset = localHeaderOffset + 30
                + UInt64(local.u16(at: 26)) + UInt64(local.u16(at: 28))
            entries.append(entry)
            cursor += 46 + nameLength + extraLength + commentLength
        }
        return entries
    }

    /// A zip entry name as a path inside the destination, or `nil` when it
    /// would escape it (absolute path, `..` component).
    private static func safeRelativePath(_ name: String) -> String? {
        guard !name.hasPrefix("/"), !name.hasPrefix("\\") else { return nil }
        let parts = name.split(separator: "/", omittingEmptySubsequences: false)
        for part in parts where part == ".." {
            return nil
        }
        return name
    }

    // MARK: - Entry data

    private static let chunkSize = 1 << 20

    private static func copyStored(
        _ input: FileHandle, offset: UInt64, size: UInt64, to output: FileHandle,
        progress: inout Int64, report: (Int64) -> Void
    ) throws {
        try input.seek(toOffset: offset)
        var remaining = size
        while remaining > 0 {
            let want = min(remaining, UInt64(chunkSize))
            guard let data = try input.read(upToCount: Int(want)), data.count == want else {
                throw ZipError.unreadable
            }
            try output.write(contentsOf: data)
            remaining -= want
            progress += Int64(data.count)
            report(progress)
        }
    }

    private static func inflateEntry(
        _ input: FileHandle, offset: UInt64, size: UInt64, to output: FileHandle,
        progress: inout Int64, report: (Int64) -> Void
    ) throws {
        try input.seek(toOffset: offset)
        var stream = z_stream()
        stream.zalloc = nil
        stream.zfree = nil
        stream.opaque = nil
        // Raw DEFLATE (zip method 8 has no zlib header): negative window bits.
        guard inflateInit2_(&stream, -MAX_WBITS, ZLIB_VERSION, Int32(MemoryLayout<z_stream>.size)) == Z_OK else {
            throw ZipError.unsupported
        }
        defer { inflateEnd(&stream) }
        let inBuffer = UnsafeMutablePointer<UInt8>.allocate(capacity: chunkSize)
        let outBuffer = UnsafeMutablePointer<UInt8>.allocate(capacity: chunkSize)
        defer {
            inBuffer.deallocate()
            outBuffer.deallocate()
        }
        var remaining = size
        var finished = false
        while !finished {
            if stream.avail_in == 0 && remaining > 0 {
                let want = min(remaining, UInt64(chunkSize))
                guard let data = try input.read(upToCount: Int(want)), data.count == want else {
                    throw ZipError.unreadable
                }
                data.copyBytes(to: inBuffer, count: data.count)
                stream.next_in = inBuffer
                stream.avail_in = uInt(data.count)
                remaining -= UInt64(data.count)
            }
            stream.next_out = outBuffer
            stream.avail_out = uInt(chunkSize)
            let result = inflate(&stream, Z_NO_FLUSH)
            if result != Z_OK && result != Z_STREAM_END && result != Z_BUF_ERROR {
                throw ZipError.unreadable
            }
            let produced = chunkSize - Int(stream.avail_out)
            if produced > 0 {
                try output.write(contentsOf: Data(bytes: outBuffer, count: produced))
                progress += Int64(produced)
                report(progress)
            }
            if result == Z_STREAM_END {
                finished = true
            } else if result == Z_BUF_ERROR && stream.avail_in == 0 && remaining == 0 {
                throw ZipError.unreadable
            }
        }
    }

    enum ZipError: LocalizedError {
        case unreadable
        case unsupported
        case unsafeEntry
        var errorDescription: String? {
            switch self {
            case .unreadable: return Strings.zipUnreadable
            case .unsupported: return Strings.zipEntryNotSupported
            case .unsafeEntry: return Strings.zipEntryUnsafe
            }
        }
    }
}

// MARK: - Little-endian reads

private extension Data {
    func u16(at offset: Int) -> UInt16 {
        UInt16(self[offset]) | (UInt16(self[offset + 1]) << 8)
    }

    func u32(at offset: Int) -> UInt32 {
        (0..<4).reduce(UInt32(0)) { $0 | (UInt32(self[offset + $1]) << (8 * $1)) }
    }

    func u64(at offset: Int) -> UInt64 {
        (0..<8).reduce(UInt64(0)) { $0 | (UInt64(self[offset + $1]) << (8 * $1)) }
    }
}
