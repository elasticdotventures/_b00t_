#!/usr/bin/env bats

# submodule-drift.bats — fixture tests for check-submodule-drift.sh (#923)
#
# Builds a fully throwaway superproject + submodule fixture per test (never
# touches this repo's real vendor submodules). Verifies the classification
# of OK / UNINIT / ORPHANED / DRIFTED+CLEAN / DRIFTED+DIRTY, and that --fix
# only ever moves drifted+CLEAN submodules, never drifted+dirty ones.

load 'test_helper/bats-support/load'
load 'test_helper/bats-assert/load'

# The real script this repo ships, resolved relative to this test file
# (_b00t_/test/submodule-drift.bats -> _b00t_/scripts/check-submodule-drift.sh).
REAL_SCRIPT="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/scripts/check-submodule-drift.sh"

# Look up one submodule's classified status from a --json run's output.
status_of() {
    local json="$1" path="$2"
    echo "$json" | jq -r --arg p "$path" '.[] | select(.path==$p) | .status'
}

setup() {
    FIXTURE_DIR="$(mktemp -d)"

    # 1. Source repo for submodules: two real commits, c1 -> c2, both
    #    touching the SAME tracked file so checking out c1 and editing it
    #    exercises a genuine tracked-file dirty state.
    SRC="$FIXTURE_DIR/fixture-sub-src"
    git init -q "$SRC"
    git -C "$SRC" config user.email test@example.com
    git -C "$SRC" config user.name test
    git -C "$SRC" config commit.gpgsign false
    echo "v1" > "$SRC/file.txt"
    git -C "$SRC" add file.txt
    git -C "$SRC" commit -q -m c1
    C1_SHA="$(git -C "$SRC" rev-parse HEAD)"
    echo "v2" > "$SRC/file.txt"
    git -C "$SRC" commit -q -am c2
    C2_SHA="$(git -C "$SRC" rev-parse HEAD)"

    # 2. Origin superproject: sub-a, sub-b, sub-c, sub-d all pinned @c2.
    #    Modern git disables the file:// submodule transport by default —
    #    protocol.file.allow=always is required for `submodule add`/`update`.
    ORIGIN="$FIXTURE_DIR/fixture-super-origin"
    git init -q "$ORIGIN"
    git -C "$ORIGIN" config user.email test@example.com
    git -C "$ORIGIN" config user.name test
    git -C "$ORIGIN" config commit.gpgsign false
    for name in sub-a sub-b sub-c sub-d; do
        git -C "$ORIGIN" -c protocol.file.allow=always submodule add -q "$SRC" "$name"
    done
    git -C "$ORIGIN" commit -q -m "pin submodules @c2"

    # 3. Fresh plain clone (NOT --recurse-submodules) -> every submodule
    #    starts genuinely UNINITIALIZED (empty dir, no .git inside).
    SUPER="$FIXTURE_DIR/fixture-super"
    git clone -q "$ORIGIN" "$SUPER"

    # 4. Drop the real script in at the same repo-relative location it lives
    #    at in this repo, so its own BASH_SOURCE-based REPO_ROOT resolution
    #    (two levels up from _b00t_/scripts/) resolves to $SUPER, not here.
    mkdir -p "$SUPER/_b00t_/scripts"
    cp "$REAL_SCRIPT" "$SUPER/_b00t_/scripts/check-submodule-drift.sh"
    chmod +x "$SUPER/_b00t_/scripts/check-submodule-drift.sh"
    SCRIPT="$SUPER/_b00t_/scripts/check-submodule-drift.sh"

    export FIXTURE_DIR SRC ORIGIN SUPER SCRIPT C1_SHA C2_SHA
}

teardown() {
    rm -rf "$FIXTURE_DIR"
}

@test "uninitialized: never-initialized submodules are classified uninit, never a failure" {
    # Nothing initialized at all — the state right after a plain clone.
    run bash "$SCRIPT" --json
    assert_success
    for name in sub-a sub-b sub-c sub-d; do
        assert_equal "$(status_of "$output" "$name")" "uninit"
    done

    run bash "$SCRIPT"
    assert_success
    assert_output --partial "PASS: 0 drifted submodules"
}

@test "orphaned: stale .gitmodules entry is skipped, not a false drift, doesn't crash" {
    cat >> "$SUPER/.gitmodules" <<EOF
[submodule "sub-orphan"]
	path = sub-orphan
	url = $SRC
EOF

    run bash "$SCRIPT" --json
    assert_success
    assert_equal "$(status_of "$output" "sub-orphan")" "orphaned"

    run bash "$SCRIPT"
    assert_success
    assert_output --partial "PASS: 0 drifted submodules"
}

@test "clean-drift: drifted+clean submodule is reported, then --fix syncs it to the recorded pin" {
    git -C "$SUPER" -c protocol.file.allow=always submodule update --init -- sub-a
    git -C "$SUPER/sub-a" checkout -q "$C1_SHA"

    run bash "$SCRIPT" --json
    assert_equal "$status" 1
    assert_equal "$(status_of "$output" "sub-a")" "drifted_clean"

    run bash "$SCRIPT" --fix --json
    assert_success
    assert_equal "$(status_of "$output" "sub-a")" "drifted_fixed"
    assert_equal "$(git -C "$SUPER/sub-a" rev-parse HEAD)" "$C2_SHA"
}

@test "dirty-drift: drifted+dirty submodule is reported, --fix never touches it" {
    git -C "$SUPER" -c protocol.file.allow=always submodule update --init -- sub-b
    git -C "$SUPER/sub-b" checkout -q "$C1_SHA"
    echo "local-uncommitted-edit" > "$SUPER/sub-b/file.txt"

    run bash "$SCRIPT" --json
    assert_equal "$status" 1
    assert_equal "$(status_of "$output" "sub-b")" "drifted_dirty"

    run bash "$SCRIPT" --fix --json
    assert_equal "$status" 1
    assert_equal "$(status_of "$output" "sub-b")" "drifted_dirty"
    # HEAD must remain untouched at c1, and the uncommitted edit must survive.
    assert_equal "$(git -C "$SUPER/sub-b" rev-parse HEAD)" "$C1_SHA"
    run cat "$SUPER/sub-b/file.txt"
    assert_output "local-uncommitted-edit"
}

@test "untracked-noise: untracked files alone do not count as dirty (regression for tracked-only definition)" {
    git -C "$SUPER" -c protocol.file.allow=always submodule update --init -- sub-c
    git -C "$SUPER/sub-c" checkout -q "$C1_SHA"
    touch "$SUPER/sub-c/newfile.txt"

    run bash "$SCRIPT" --json
    assert_equal "$status" 1
    # Classified CLEAN despite the untracked file — proves "dirty" means
    # tracked changes only (--untracked-files=no), not "any untracked file".
    assert_equal "$(status_of "$output" "sub-c")" "drifted_clean"

    run bash "$SCRIPT" --fix --json
    assert_success
    assert_equal "$(status_of "$output" "sub-c")" "drifted_fixed"
    assert_equal "$(git -C "$SUPER/sub-c" rev-parse HEAD)" "$C2_SHA"
    # The untracked file survives the sync (git checkout never touches it).
    assert [ -f "$SUPER/sub-c/newfile.txt" ]
}
