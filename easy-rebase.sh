
# find common ancestor (set this to the branch you want to rebase on)
# BASE=next/r4

# Check for an open PR using GitHub CLI
PR_BASE=$(gh pr view --json baseRefName --jq '.baseRefName' 2>/dev/null)

# If no PR is found, exit the script
if [ -z "$PR_BASE" ]; then
    echo "No open pull request found for branch $CHANGED. Cowardly refusing to proceed."
    exit 1
fi

# use current branch
CHANGED=$(git rev-parse --abbrev-ref HEAD)

COMMON_ANCESTOR=$(git merge-base $PR_BASE $CHANGED)

COUNT=$(git rev-list --count $COMMON_ANCESTOR..$CHANGED)
echo "There are $COUNT commits between $BASE $COMMON_ANCESTOR and $CHANGED"

# Start interactive rebase
git rebase -i HEAD~$COUNT

# Edit the commit message 
msg=$(read -p "write something inspiring that summarizes the progress")
echo git commit --amend -m "🎂 $msg"

# Force push the changes (if the branch is already pushed to a remote)
echo git push --force

