#!/usr/bin/env bash
# 🥾 Zellij Interactive Modal Dialog
# Displays a confirmation dialog in a floating pane.
# The user MUST acknowledge the dialog for success.
#
# Usage: zellij-modal <title> <message>
#   or just: zellij-modal
#
# Exit codes:
#   0 - User confirmed (Yes)
#   1 - User declined (No)
#   2 - Timed out / Error

set -euo pipefail

TITLE="${1:-🥾 B00T Interactive Modal}"
MESSAGE="${2:-This is an interactive Zellij modal dialog.\n\nDo you confirm this action?}"
RESULT_FILE="/tmp/b00t-modal-result-$$"
rm -f "$RESULT_FILE"

echo -e "\033[1;34m🥾 B00T Interactive Modal\033[0m"
echo -e "\033[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\033[0m"
echo ""
echo -e "  \033[1;37m$TITLE\033[0m"
echo ""
echo -e "  $MESSAGE"
echo ""
echo -e "\033[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\033[0m"
echo ""
echo -e "  \033[32m[Y] Yes\033[0m  \033[31m[N] No\033[0m  \033[33m[C] Cancel\033[0m"
echo ""
printf "  \033[1;33m❯\033[0m "
read -r -n 1 RESPONSE
echo ""

case "$RESPONSE" in
    [Yy])
        echo ""
        echo -e "  \033[32m✅ Confirmed!\033[0m"
        echo "$TITLE:YES" > "$RESULT_FILE"
        exit 0
        ;;
    [Nn])
        echo ""
        echo -e "  \033[31m❌ Declined.\033[0m"
        echo "$TITLE:NO" > "$RESULT_FILE"
        exit 1
        ;;
    [Cc])
        echo ""
        echo -e "  \033[33m⚠️  Cancelled.\033[0m"
        echo "$TITLE:CANCELLED" > "$RESULT_FILE"
        exit 1
        ;;
    *)
        echo ""
        echo -e "  \033[31m⚠️  Invalid: $RESPONSE\033[0m"
        echo "$TITLE:INVALID" > "$RESULT_FILE"
        exit 2
        ;;
esac
