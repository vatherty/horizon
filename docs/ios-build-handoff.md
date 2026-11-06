# Patch application and IPA build handoff

This is for the build/upload agent. The player only needs Safari, Files and
SideStore on their iPhone. Do not ask the player to install Git, Rust or Xcode.

Repository: https://github.com/vatherty/horizon
Original branch: ios-port
Patch base: 8dcf7f78e91e6f1a504516db0dbc90652480b0fc
Initial checkout before remote advancement: 88231c8b0953a60a1361b071e0a38574ad77348a

The delivery ZIP includes a cumulative patch and an ordered git-format-patch
series. Apply ONE form, not both. Prefer the series to retain reviewable commits
and agent attribution. The bundle records the resulting source HEAD/tree and
SHA-256 hashes. No Minecraft resources or player saves are included.

## Apply in a cloud checkout

Fetch and inspect the current remote first. Preserve any newer ios-port changes,
including CI fixes. Create a new integration branch; never reset/rebase/force-push
an existing shared branch.

For the exact base in a fresh checkout:

```sh
git clone --branch ios-port https://github.com/vatherty/horizon.git
cd horizon
git switch -c ios-performance-review 8dcf7f78e91e6f1a504516db0dbc90652480b0fc
git am /path/to/unpacked/series/*.patch
```

If ios-port has advanced, branch from its current head and use git am --3way,
reviewing any conflicts before continuing. Alternatively apply the series on the
recorded base and merge the newer remote branch. Inspect the final diff and run
checks again. Do not discard intervening commits to make the patches apply.

For the cumulative patch on the exact base, use git apply --check first, then
git apply --index and make a reviewed commit. It has the same source tree as the
series, but does not preserve its commit sequence. The delivery was verified by
applying both forms to separate clean base worktrees and comparing trees; see
validation results for the actual outcome.

## Validate and build on Apple's SDK

This implementation environment is Linux. A new validation-branch push was
rejected by GitHub with HTTP403 (permission denied), so the patched revision
could not be submitted to macOS Actions here. An earlier successful shell build
is not validation of these changes. The upload agent needs authorized repository
write/Actions access; no Apple signing credentials are needed for the unsigned IPA.

Push a new reviewed branch using authorized access, then dispatch:

```sh
gh workflow run ios.yml --repo vatherty/horizon \
  --ref YOUR_REVIEW_BRANCH -f publish_release=false
```

Keep the newer remote macos-latest runner selection (8dcf7f78e). The workflow checks the public tree, iOS crate
formatting, existing Metal backend host regression, cargo check/clippy for
aarch64-apple-ios, then builds/links the real Swift/C/Rust app with XcodeGen and
xcodebuild. The conditional memory shim must compile against Apple Mach/os
headers; PlatformPowerMonitor and its exported Rust symbols must typecheck and
link. Do not accept a cfg-disabled Linux iOS test or an unsigned staticlib alone.
The IPA engine-only content check must pass before distribution.

Read the validation report for host checks and unresolved harness requirements.
Use fresh scratch homes/worlds for any additional runs, as AGENTS.md requires.
Run prepared-resource server/placement/save/inventory/reload and LAN harnesses
where lawful prepared data are available; never upload those data with patches.

After the SDK check, clippy and complete IPA link succeed, upload the matching
IPA artifact and send the player its commit-specific download link. An ordinary
push to ios-port still updates ios-latest; workflow_dispatch defaults to artifact
only. Explicitly enable publish_release only when intentionally replacing that
fixed download with the tested revision. Do not publish an old/base IPA as though
it contained these patches. Keep source revision and IPA revision linked.

## Player handoff

Send the player the IPA link and [phone acceptance steps](ios-phone-tests.md).
They download in Safari to Files, sign/install through SideStore, import their
lawful prepared-resource archive in Horizon if needed, close and reopen. Arrange
that archive through an authorized source/build provider if it is not already on
the phone; the app cannot prepare Java resources locally.

The Files summary is Documents/performance-summary.json; logs/latest.log,
crash-reports, optional logs/startup.jsonl and screenshots supplement it. Perform
cold and warmed runs with matching view/settings/caps and record OS power/thermal
state. Experiments stay off until device correctness and measurements justify
otherwise. There are no claimed FPS, RAM, thermal or battery improvements from
host tests alone.
