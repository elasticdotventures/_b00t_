import hashlib
from pathlib import Path

from checkpoint_manifest import CheckpointSpec
from populate_volume import is_present, populate


def make_spec(tmp_relpath: str, content: bytes, sha256: str | None) -> CheckpointSpec:
    return CheckpointSpec(
        url=f"https://example.invalid/{tmp_relpath}",
        relpath=tmp_relpath,
        size_bytes=len(content),
        sha256=sha256,
    )


def test_is_present_false_when_file_missing(tmp_path):
    spec = make_spec("missing.bin", b"x" * 10, None)
    assert is_present(tmp_path, spec) is False


def test_is_present_false_when_size_mismatch(tmp_path):
    spec = make_spec("partial.bin", b"x" * 100, None)
    (tmp_path / "partial.bin").write_bytes(b"x" * 40)
    assert is_present(tmp_path, spec) is False


def test_is_present_true_when_size_matches_and_no_hash_expected(tmp_path):
    content = b"x" * 50
    spec = make_spec("ok.bin", content, None)
    (tmp_path / "ok.bin").write_bytes(content)
    assert is_present(tmp_path, spec) is True


def test_is_present_false_when_hash_mismatch(tmp_path):
    content = b"x" * 50
    real_sha = hashlib.sha256(content).hexdigest()
    spec = make_spec("hashed.bin", content, real_sha)
    (tmp_path / "hashed.bin").write_bytes(b"y" * 50)  # same size, wrong content
    assert is_present(tmp_path, spec) is False


def test_is_present_true_when_hash_matches(tmp_path):
    content = b"x" * 50
    real_sha = hashlib.sha256(content).hexdigest()
    spec = make_spec("hashed_ok.bin", content, real_sha)
    (tmp_path / "hashed_ok.bin").write_bytes(content)
    assert is_present(tmp_path, spec) is True


def test_populate_skips_already_present_files(tmp_path):
    content = b"x" * 20
    spec = make_spec("already.bin", content, None)
    (tmp_path / "already.bin").write_bytes(content)

    fetch_calls = []

    def fake_fetch(url, dest):
        fetch_calls.append(url)

    fetched = populate(tmp_path, [spec], fetch=fake_fetch)
    assert fetched == []
    assert fetch_calls == []


def test_populate_fetches_missing_files(tmp_path):
    content = b"x" * 20
    spec = make_spec("missing2.bin", content, None)

    def fake_fetch(url, dest):
        Path(dest).write_bytes(content)

    fetched = populate(tmp_path, [spec], fetch=fake_fetch)
    assert fetched == ["missing2.bin"]
    assert (tmp_path / "missing2.bin").read_bytes() == content
