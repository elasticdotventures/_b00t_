#!/usr/bin/env python3

import json
import tempfile
import unittest
from pathlib import Path

from scripts.generate_claude_marketplace import generate


class GenerateMarketplaceTest(unittest.TestCase):
    def _write_minimal_fixtures(self, root: Path) -> tuple[Path, Path]:
        """Write an empty-registry + no-bundles fixture; return (registry_path, roles_path)."""
        (root / ".claude-plugin").mkdir(parents=True, exist_ok=True)
        roles_cfg = {
            "marketplace": {
                "name": "b00t-plugins",
                "owner": "elasticdotventures",
                "description": "test",
                "version": "0.2.0",
                "pluginRoot": "./plugins",
            },
            "base_plugins": [],
            "bundles": [],
        }
        registry_path = root / "mcp_registry.json"
        roles_path = root / "config" / "claude-marketplace-roles.json"
        roles_path.parent.mkdir(parents=True, exist_ok=True)
        registry_path.write_text(json.dumps({}), encoding="utf-8")
        roles_path.write_text(json.dumps(roles_cfg), encoding="utf-8")
        return registry_path, roles_path

    def test_generates_role_recipe_and_marketplace(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            (root / ".claude-plugin").mkdir(parents=True)

            registry = {
                "local.b00t/fetch-url-as-markdown": {
                    "id": "local.b00t/fetch-url-as-markdown",
                    "name": "fetch-url-as-markdown",
                    "description": "Fetches a URL and converts content to markdown",
                    "version": "0.1.0",
                    "tags": ["http"],
                    "config": {
                        "command": "uvx",
                        "args": ["fetch-url-as-markdown"],
                        "transport": "stdio",
                    },
                }
            }
            roles_cfg = {
                "marketplace": {
                    "name": "b00t-plugins",
                    "owner": "elasticdotventures",
                    "description": "test",
                    "version": "0.2.0",
                    "pluginRoot": "./plugins",
                },
                "base_plugins": [
                    {
                        "name": "b00t",
                        "source": "./plugins/b00t",
                        "description": "base",
                        "version": "0.1.0",
                    }
                ],
                "bundles": [
                    {
                        "type": "skill",
                        "id": "document-understanding",
                        "name": "Document Understanding",
                        "description": "role desc",
                        "tags": ["docling"],
                        "servers": [
                            {"registry_id": "local.b00t/fetch-url-as-markdown"},
                            {
                                "id": "external.docling/docling-mcp",
                                "name": "docling",
                                "description": "Docling MCP",
                                "config": {
                                    "command": "uvx",
                                    "args": ["--from", "docling-mcp", "docling-mcp-server"],
                                    "transport": "stdio",
                                },
                            },
                        ],
                    }
                ],
            }

            registry_path = root / "mcp_registry.json"
            roles_path = root / "config" / "claude-marketplace-roles.json"
            roles_path.parent.mkdir(parents=True, exist_ok=True)
            registry_path.write_text(json.dumps(registry), encoding="utf-8")
            roles_path.write_text(json.dumps(roles_cfg), encoding="utf-8")

            rc = generate(root, registry_path, roles_path, check=False)
            self.assertEqual(rc, 0)

            marketplace = json.loads((root / ".claude-plugin" / "marketplace.json").read_text())
            names = [p["name"] for p in marketplace["plugins"]]
            self.assertIn("skill-document-understanding", names)

            role_recipe = json.loads(
                (root / ".claude-plugin" / "recipes" / "skills" / "document-understanding.json").read_text()
            )
            self.assertIn("docling", role_recipe["mcpServers"])
            self.assertIn("fetch-url-as-markdown", role_recipe["mcpServers"])

            rc_check = generate(root, registry_path, roles_path, check=True)
            self.assertEqual(rc_check, 0)

    def test_check_detects_stale_files(self) -> None:
        """check=True must return 1 when an unexpected (stale) file exists in a generated dir."""
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            registry_path, roles_path = self._write_minimal_fixtures(root)

            rc = generate(root, registry_path, roles_path, check=False)
            self.assertEqual(rc, 0)

            # Inject a stale file that would not be regenerated.
            stale = root / ".claude-plugin" / "recipes" / "mcp-servers" / "stale-old-recipe.json"
            stale.write_text('{"stale": true}\n', encoding="utf-8")

            rc_check = generate(root, registry_path, roles_path, check=True)
            self.assertEqual(rc_check, 1, "check must fail when stale files are present")

    def test_check_detects_missing_expected_file(self) -> None:
        """check=True must return 1 when an expected output file has been deleted."""
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            registry_path, roles_path = self._write_minimal_fixtures(root)

            rc = generate(root, registry_path, roles_path, check=False)
            self.assertEqual(rc, 0)

            # Remove an expected output file.
            (root / ".claude-plugin" / "marketplace.json").unlink()

            rc_check = generate(root, registry_path, roles_path, check=True)
            self.assertEqual(rc_check, 1, "check must fail when an expected file is missing")


if __name__ == "__main__":
    unittest.main()
