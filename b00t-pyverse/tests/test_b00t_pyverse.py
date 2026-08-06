"""Tests for b00t-pyverse native Rust bindings.

Run: cd b00t-pyverse && python -m pytest tests/ -v
"""

import importlib
import json

import pytest

# The native extension's actual filename varies by build (Python-version-
# tagged for a version-specific build, `_core.abi3.so` for an abi3 build) —
# import by module name rather than hardcoding a filename, exactly like
# b00t_pyverse/__init__.py's own _get_core() does.
native = importlib.import_module("b00t_pyverse._core")


class TestGuardCheck:
    def test_guard_check_pip_install(self):
        result = json.loads(native.guard_check("pip install flask"))
        assert result["action"] in ("warn", "block")

    def test_guard_check_safe_command(self):
        result = json.loads(native.guard_check("ls -la"))
        assert result["action"] == "allow"

    def test_guard_check_custom_guards(self):
        guards = json.dumps([
            {"pattern": "docker run", "action": "block",
             "message": "use podman", "redirect": None, "repeat_threshold": None}
        ])
        result = json.loads(native.guard_check("docker run nginx", guards))
        assert result["action"] == "block"

    def test_guard_coverage(self):
        result = native.guard_coverage()
        assert isinstance(result, str) and len(result) > 0


class TestEmojiRegistry:
    def test_lookup_shortcode(self):
        r = json.loads(native.emoji_lookup(":skunk:"))
        assert r["found"] and r["literal"] == "\U0001f9a8" and r["tier"] == 1

    def test_lookup_g0spell(self):
        r = json.loads(native.emoji_lookup("antipattern"))
        assert r["found"] and r["literal"] == "\U0001f4a9" and r["tier"] == 2

    def test_lookup_literal(self):
        r = json.loads(native.emoji_lookup("\U0001f6ab"))
        assert r["found"] and r["g0spell"] == "block"

    def test_lookup_not_found(self):
        r = json.loads(native.emoji_lookup(":nope:"))
        assert r["found"] is False

    def test_list_entries(self):
        r = json.loads(native.emoji_list())
        assert isinstance(r, list) and len(r) >= 9

    def test_list_has_skunk_and_poop(self):
        gs = {e["g0spell"] for e in json.loads(native.emoji_list())}
        assert "skunk" in gs and "antipattern" in gs


class TestParser:
    def test_parse_slash(self):
        r = json.loads(native.parse_k0mmand3r("/deploy --env=prod"))
        assert "verb" in r

    def test_parse_no_verb(self):
        r = json.loads(native.parse_k0mmand3r("plain text"))
        assert isinstance(r, dict)

    def test_parse_empty(self):
        r = json.loads(native.parse_k0mmand3r(""))
        assert isinstance(r, dict)


class TestStageGuard:
    def test_register_valid(self):
        r = json.loads(native.register_stage_guard_py("pre_parse", lambda s: "allow"))
        assert r["registered"] and r["stage"] == "pre_parse"

    def test_register_invalid(self):
        with pytest.raises(Exception, match="Unknown stage"):
            native.register_stage_guard_py("bad_stage", lambda s: "allow")

    def test_register_all_stages(self):
        for s in ["pre_parse","pre_verb","post_verb","pre_params","post_params",
                   "pre_content","post_content","post_parse"]:
            r = json.loads(native.register_stage_guard_py(s, lambda s_: "block"))
            assert r["registered"], f"Stage {s} failed"


class TestVersion:
    def test_version(self):
        assert isinstance(native.version(), str) and len(native.version()) > 0
