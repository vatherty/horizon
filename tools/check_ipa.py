#!/usr/bin/env python3
"""Fail if an iOS .ipa holds anything but the engine.

The .ipa may contain the app bundle only: its executable, Info.plist, launch
screen, asset catalog and icon. It must never contain Minecraft's assets or
data, prepared resources, game jars or sounds: those are prepared on the
player's computer from their own copy and imported on the phone.
"""

import argparse
from pathlib import PurePosixPath
import sys
import zipfile

# Exactly this top-level directory, holding one .app.
TOP_LEVEL_DIRECTORY = "Payload"
# Files a bare .app holds at its top level, besides its executable.
APP_TOP_LEVEL_FILES = {"Info.plist", "PkgInfo", "Assets.car", "embedded.mobileprovision"}
# Directories a bare .app holds, with what they may hold.
APP_DIRECTORIES = {"_CodeSignature", "Frameworks"}
# Suffixes a bare .app file may carry (the executable itself has none).
APP_FILE_SUFFIXES = {".png", ".jpg", ".jpeg", ".plist", ".strings", ".car", ".ttf", ".otf", ".nib"}
# Anything that looks like game content, wherever it sits.
FORBIDDEN_NAMES = ("horizon-resources.json", "level.dat", "options.txt", "horizon-data.bin")
FORBIDDEN_PARTS = {"assets", "minecraft", "sounds", "resources", "data", ".local", "decompiled"}
FORBIDDEN_SUFFIXES = {".jar", ".class", ".ogg", ".mcmeta", ".nbt", ".mca", ".json", ".lang", ".zip"}
# A generous bound: the engine is a few tens of megabytes; game content would dwarf it.
MAX_BYTES = 250 * 1024 * 1024


def problems(names, size):
    found = []
    if size > MAX_BYTES:
        found.append(f"the .ipa is {size >> 20} MiB, more than the {MAX_BYTES >> 20} MiB an engine-only .ipa needs")
    for name in names:
        path = PurePosixPath(name)
        if name.endswith("/"):
            continue
        parts = path.parts
        if parts[0] != TOP_LEVEL_DIRECTORY:
            found.append(f"{name}: outside {TOP_LEVEL_DIRECTORY}/")
            continue
        if len(parts) < 2 or not parts[1].endswith(".app"):
            found.append(f"{name}: {TOP_LEVEL_DIRECTORY}/ holds only the .app")
            continue
        app = parts[1][:-4]
        rest = parts[2:]
        if not rest:
            found.append(f"{name}: not an engine file")
            continue
        if rest[0] in APP_DIRECTORIES:
            if rest[0] == "Frameworks" and (
                len(rest) < 2 or not rest[1].endswith(".framework")
            ):
                found.append(f"{name}: only Frameworks/*.framework/ belongs in Frameworks/")
        elif rest[0].endswith((".lproj", ".storyboardc", ".bundle")):
            pass
        elif len(rest) == 1 and (
            rest[0] in APP_TOP_LEVEL_FILES
            or (not path.suffix and rest[0] == app)
            or path.suffix.lower() in APP_FILE_SUFFIXES
        ):
            pass
        else:
            found.append(f"{name}: outside the places an engine-only .ipa uses")
            continue
        if (
            path.name.lower() in FORBIDDEN_NAMES
            or path.suffix.lower() in FORBIDDEN_SUFFIXES
            or any(part.lower() in FORBIDDEN_PARTS for part in parts[:-1])
        ):
            found.append(f"{name}: looks like game content")
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("ipa")
    args = parser.parse_args()
    with zipfile.ZipFile(args.ipa) as ipa:
        names = ipa.namelist()
        size = sum(info.file_size for info in ipa.infolist())
    errors = problems(names, size)
    if errors:
        print(f"{args.ipa} holds more than the engine:", file=sys.stderr)
        for error in sorted(set(errors)):
            print(f"  {error}", file=sys.stderr)
        return 1
    print(f"{args.ipa}: {len(names)} entries, engine only.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
