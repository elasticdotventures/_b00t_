"""Tests for b00t-py native Rust bindings.

Loads the native .so module directly under the correct name
to bypass __init__.py circular import issues.

Run: cd b00t-py && python -m pytest tests/ -v
"""

import importlib.machinery
import importlib.util
import json
import os
import sys

# Load the native .so with the CORRECT module name that the .so expects
_so_dir = os.path.join(os.path.dirname(__file__), "..", "python", "b00t_py")
_so_path = os.path.join(_so_dir, "_core.cpython-313-x86_64-linux-gnu.so")

# Insert the module into sys.modules under its expected name first
loader = importlib.machinery.ExtensionFileLoader("b00t_py._core", _so_path)
spec = importlib.machinery.ModuleSpec(name="b00t_py._core", loader=loader, origin=_so_path)
native = importlib.util.module_from_spec(spec)
sys.modules["b00t_py._core"] = native
loader.exec_module(native)

import pytest


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
