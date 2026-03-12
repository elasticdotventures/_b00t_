tips and tricks
Instead of:

tmpfile=$(mktemp)
# do stuff with $tmpfile
rm "$tmpfile"
# hope nothing failed before we got here
Just use:

cleanup() { rm -f "$tmpfile"; }
trap cleanup EXIT

tmpfile=$(mktemp)
# do stuff with $tmpfile
trap runs your function no matter how the script exits -- normal, error, Ctrl+C, kill. Your temp files always get cleaned up. No more orphaned junk in /tmp.

Real world:

# Lock file that always gets released
cleanup() { rm -f /var/run/myapp.lock; }
trap cleanup EXIT
touch /var/run/myapp.lock

# SSH tunnel that always gets torn down
cleanup() { kill "$tunnel_pid" 2>/dev/null; }
trap cleanup EXIT
ssh -fN -L 5432:db:5432 jumpbox &
tunnel_pid=$!

# Multiple things to clean up
cleanup() {
    rm -f "$tmpfile" "$pidfile"
    kill "$bg_pid" 2>/dev/null
}
trap cleanup EXIT
The trick is defining trap before creating the resources. If your script dies between mktemp and the rm at the bottom, the file stays. With trap at the top, it never does.

Works in bash, zsh, and POSIX sh. One of the few tricks that's actually portable.

