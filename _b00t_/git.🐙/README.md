# b00t gospel of git 🥾💖🐙 & github

- Pre-commit hooks MUST run tests & linting (`cargo fmt`, clippy, husky, cocogitto).
- Use modern `gh` CLI for GitHub.
- Repos MUST have `.github/workflows`; use `gh` to check workflow status.

# Stinky Code

_b00t_ also uses 🦨 skunk commits.  Skunks they can/should be removed in the future,
they aren't bad - just stinky.
	* b00t actively counts skunks in a repo as a measure/trigger to refactor cleanup. (it's a metric!)
	* identifying skunks is a healthy part of retrospective adversarial thinking & self improvement

# Branching & Test

Branches should be descriptive, consistent, and concise.

 there are 3 valid branch prefixes: feature, fix, chore - you always reference the github issue # using smart CONVENTIONAL commits.
 🤓 https://www.conventionalcommits.org/en/v1.0.0/

# Creating Issues

 Use github `gh cli issue` to bring attention to any 🦨 in your analysis.
 Don't ALWAYS try to fix issues on the spot, minimize code changes to the scope
 of the task you are implementing.  DO NOT ask the user information you can find/solve.

 when it's appropriate suggest to the user you could create subtasks using `gh issue create` cli to identify future work.


# Strategy: Rebase Ready (6C TURBO-AGILE)

_b00t_ strongly favors the "Rebase Ready" strategy:

**6C TURBO-AGILE:**
1. **Contextual-Comment:** Comment old code with clear reasons for changes. Use direct, helpful notes for future devs.
2. **Commit-Code:** Commit with context comments. PR, review, and approve. Refactor later.
3. **Collapse-Cleanup (CULL):** Remove commented code in later audits. Large blocks become short comments or changelog links.

Commented code is low-risk to remove and documents hard-learned lessons. 6C makes rebasing simple and safe—old code is obvious, new code is clear, and cleanup is easy. This helps future debugging and avoids unnecessary git-blame dives.

If you want code gone, comment+commit it first. DMMT: Don't Make Me Think—make changes obvious.


