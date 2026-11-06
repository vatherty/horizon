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

/// Where the prepared resources are, by the game's own rules (an
/// horizon-args.txt `--resources` override, the import in resources/, or the
/// prepared folder): 0 missing, 1 in resources/, 2 in the prepared folder,
/// 3 at the args override. Pure path checks — no game state is touched, so
/// the launcher may call it on any launch, as often as it likes.
int32_t horizon_ios_resources_state(const char *documents);

/// ProcessInfo thermal severity: 0 nominal, 1 fair, 2 serious, 3 critical.
/// These setters only update atomics; notification delivery never waits for
/// rendering, world ticks or disk I/O.
void horizon_ios_set_thermal_state(int32_t state);
void horizon_ios_set_low_power_mode(int32_t enabled);

#endif
