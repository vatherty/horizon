// Horizon-Bridging-Header.h
//
// The C surface Swift sees: the Rust engine's entry point, and zlib, which the
// resource importer uses to inflate the prepared-resources zip (zip entries are
// raw DEFLATE streams; see ZipExtractor.swift).
//
// The Rust function is declared in ios/src/lib.rs (`horizon_ios_main`). Its
// return codes are in ios/README.md ("The Rust entry point").

#ifndef HORIZON_BRIDGING_HEADER_H
#define HORIZON_BRIDGING_HEADER_H

#include <stdint.h>
#include <zlib.h>

/// Starts the game in the app's Documents folder. Runs the game loop on the
/// calling (main) thread and returns when the game ends: 0 ended, 1 error
/// (the message is in Documents/last-error.txt), 2 the prepared resources are
/// not on the phone yet.
int horizon_ios_main(const char *documents);

#endif
