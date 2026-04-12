"""Decision-tree support for the LangChain operator agent."""

from __future__ import annotations

import tomllib
from dataclasses import dataclass
from pathlib import Path


@dataclass(slots=True)
class DecisionRule:
    """Single routing rule for the operator agent."""

    name: str
    match_any: list[str]
    action: str
    tool: str
    script: str | None
    notes: str | None


@dataclass(slots=True)
class DecisionMatch:
    """Matched rule plus the keyword that fired it."""

    rule: DecisionRule
    keyword: str


class DecisionTree:
    """Keyword router backed by `_b00t_/operator-decision-tree.tomllm`."""

    def __init__(self, rules: list[DecisionRule]) -> None:
        self.rules = rules

    @classmethod
    def from_file(cls, path: Path) -> "DecisionTree":
        """Load rules from TOML/TOMLLM."""
        with open(path, "rb") as handle:
            data = tomllib.load(handle)

        rules: list[DecisionRule] = []
        for item in data.get("decision_tree", {}).get("rules", []):
            rules.append(
                DecisionRule(
                    name=item["name"],
                    match_any=item.get("match_any", []),
                    action=item["action"],
                    tool=item["tool"],
                    script=item.get("script"),
                    notes=item.get("notes"),
                )
            )
        return cls(rules)

    def match(self, request: str) -> DecisionMatch | None:
        """Return the first matching rule for a request."""
        normalized = request.lower()
        for rule in self.rules:
            for keyword in rule.match_any:
                if keyword.lower() in normalized:
                    return DecisionMatch(rule=rule, keyword=keyword)
        return None

    def summary(self) -> str:
        """Compact human-readable summary for prompt injection."""
        lines = ["Operator decision tree:"]
        for rule in self.rules:
            keywords = ", ".join(rule.match_any[:4])
            lines.append(
                f"- {rule.name}: action={rule.action} tool={rule.tool} keywords=[{keywords}]"
            )
        return "\n".join(lines)
