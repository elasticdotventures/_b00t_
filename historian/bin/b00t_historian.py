#!/usr/bin/env python3
"""b00t-historian: NATS hive coordination archiver.

Subscribes to a hive coordination subject and appends every message as
NDJSON to a dated session log, so hive agents have a durable, replayable
record of what happened. Rewritten from scratch (elasticdotventures/_b00t_
#1248) after this file was found committed as a 0-byte empty file in
a46152f4, despite that commit's message describing real fixes to it -
those fixes (NATS_URL env var over --nats-url on argv, credential
redaction in logs) are re-applied here, reconstructed from this file's own
past run log (historian-run.log) rather than recovered from git history -
no non-empty version of this file exists anywhere in this repo's history.

Global options (--id/--nats-url/--subject/--log-dir/--basename) work
whether given before or after the run/replay subcommand - the previously-
deployed version only accepted them before the subcommand, which is
exactly what crashed it (see historian-run.log's
"unrecognized arguments: --nats-url ...").
"""

import argparse
import asyncio
import json
import os
import re
import signal
import sys
from datetime import datetime, timezone
from pathlib import Path

import nats


def _redact_nats_url(url: str) -> str:
    """Strip embedded user:pass credentials from a nats:// URL before it's
    ever written to a log line - the exposure vector a46152f4 was fixing."""
    return re.sub(r"://[^@/]+@", "://***@", url)


def _default_log_dir() -> Path:
    return Path(__file__).resolve().parent.parent / "sessions"


def _resolve_nats_url(args: argparse.Namespace) -> str:
    # NATS_URL env var takes precedence over --nats-url on purpose: argv is
    # visible to any local user via `ps aux`, the process environment is
    # not. See a46152f4's commit message for the original exposure this
    # was fixing.
    return os.environ.get("NATS_URL") or args.nats_url or "nats://127.0.0.1:4222"


def _log_path(log_dir: Path, basename: str, when: datetime) -> Path:
    month_dir = log_dir / f"{when:%Y}" / f"{when:%m}"
    month_dir.mkdir(parents=True, exist_ok=True)
    return month_dir / f"{basename}.ndjson"


def _decode_payload(data: bytes):
    try:
        return json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return data.decode("utf-8", errors="replace")


async def run(args: argparse.Namespace) -> None:
    nats_url = _resolve_nats_url(args)
    subject = args.subject or f"hive.{args.id}.>"
    basename = args.basename or f"hive-{args.id}-coord"
    log_dir = Path(args.log_dir) if args.log_dir else _default_log_dir()
    # JWT/nkey .creds file, e.g. from capability-forge's mint_historian_creds
    # (elasticdotventures/_b00t_#1235) - falls back to CREDS_FILE env var for
    # the same reason NATS_URL is env-first: keep the path out of argv where
    # any local user could see it via `ps aux` (matters less for a path than
    # a password, but keeps both auth modes consistent). Plain user/password
    # (embedded in nats_url) is still supported when this isn't set - servers
    # not yet running in operator/JWT mode still need that path to work.
    creds = args.creds or os.environ.get("CREDS_FILE")

    async def error_cb(e):
        print(f"[nats error] {e}")

    async def disconnected_cb():
        print("[nats] disconnected — client will retry")

    async def reconnected_cb():
        print(f"[nats] reconnected to {_redact_nats_url(nats_url)}")

    async def closed_cb():
        print("[nats] connection closed")

    print(f"connecting to {_redact_nats_url(nats_url)} as {args.id!r}" + (f" (creds: {creds})" if creds else ""))
    nc = await nats.connect(
        nats_url,
        name=args.id,
        user_credentials=creds,
        reconnect_time_wait=2,
        max_reconnect_attempts=-1,
        error_cb=error_cb,
        disconnected_cb=disconnected_cb,
        reconnected_cb=reconnected_cb,
        closed_cb=closed_cb,
    )
    print(f"connected. subscribing to {subject!r}")
    print(f"camping on {subject!r} — logging to {log_dir}/<YYYY>/<MM>/{basename}.ndjson")

    count = 0

    async def handler(msg):
        nonlocal count
        count += 1
        now = datetime.now(timezone.utc)
        record = {
            "ts": now.isoformat(),
            "seq": count,
            "subject": msg.subject,
            "reply": msg.reply,
            "data": _decode_payload(msg.data),
        }
        path = _log_path(log_dir, basename, now)
        with open(path, "a") as f:
            f.write(json.dumps(record) + "\n")
        print(f"[{now.isoformat()}] #{count} {msg.subject} -> {path}")

    await nc.subscribe(subject, cb=handler)

    stop = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, stop.set)
    await stop.wait()
    # drain() requires an active connection to flush pending messages
    # before closing - it raises if we happen to be mid-reconnect right
    # when asked to stop (confirmed live: killing the NATS server then
    # immediately SIGTERM-ing this process reproduces
    # nats.errors.ConnectionReconnectingError). A shutdown request should
    # never crash regardless of connection state, so fall back to a plain
    # close() rather than leaving an unhandled exception as the exit path.
    try:
        await nc.drain()
    except nats.errors.Error as e:
        print(f"[nats] drain failed ({e}), closing instead")
        await nc.close()


def replay(args: argparse.Namespace) -> None:
    basename = args.basename or f"hive-{args.id}-coord"
    log_dir = Path(args.log_dir) if args.log_dir else _default_log_dir()
    paths = sorted(log_dir.glob(f"*/*/{basename}.ndjson"))

    if args.since:
        # Lexical comparison against the "YYYY/MM/basename.ndjson" relative
        # path - works because that format sorts chronologically as a
        # string. Not calendar-aware beyond that (e.g. "2026/9" vs
        # "2026/09" would sort wrong) - pass the zero-padded month.
        paths = [p for p in paths if str(p.relative_to(log_dir)) >= args.since]

    subject_prefix = None
    if args.subject:
        subject_prefix = args.subject.rstrip(">").rstrip(".")

    for path in paths:
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                if subject_prefix:
                    try:
                        rec = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if not rec.get("subject", "").startswith(subject_prefix):
                        continue
                print(line)


# Shared option specs: (flags, kwargs). Defined once, applied twice below
# with different `default` handling - see build_parser()'s comment for why.
_COMMON_OPTS = [
    (("--id",), dict(help="This historian instance's id (derives default subject/basename)")),
    (("--nats-url",), dict(help="NATS server URL (falls back to NATS_URL env var, then nats://127.0.0.1:4222)")),
    (("--subject",), dict(help="NATS subject to subscribe/filter (default: hive.{id}.>)")),
    (("--log-dir",), dict(help="Root directory for NDJSON session logs (default: historian/sessions)")),
    (("--basename",), dict(help="Log file basename (default: hive-{id}-coord)")),
    (("--creds",), dict(help="Path to a NATS .creds file (JWT + nkey seed) for operator/JWT-mode servers - see elasticdotventures/_b00t_#1235. Falls back to CREDS_FILE env var. Omit for plain user/password auth (embedded in --nats-url)")),
]

_ID_DEFAULT = os.environ.get("HISTORIAN_ID", "hive")


def _add_common_opts(target: argparse.ArgumentParser, *, suppress: bool) -> None:
    # argparse gotcha (confirmed live - the previous version of this fix
    # was silently broken by it): when the SAME --flag is defined on both
    # the top-level parser and a subparser via `parents=`, and the flag is
    # only given BEFORE the subcommand (not repeated after it), the
    # subparser's own default for that flag overwrites whatever the
    # top-level parser already set in the Namespace - even though the user
    # never touched it a second time. Fix: the top-level parser gets real
    # defaults; every subparser gets `default=argparse.SUPPRESS` for the
    # same flags, so if a subparser doesn't see the flag again, it leaves
    # the Namespace attribute alone instead of clobbering it.
    for flags, kwargs in _COMMON_OPTS:
        kwargs = dict(kwargs)
        if suppress:
            kwargs["default"] = argparse.SUPPRESS
        elif flags == ("--id",):
            kwargs["default"] = _ID_DEFAULT
        else:
            kwargs["default"] = None
        target.add_argument(*flags, **kwargs)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="b00t-historian: NATS hive coordination archiver",
    )
    _add_common_opts(parser, suppress=False)
    sub = parser.add_subparsers(dest="command", required=True)

    run_parser = sub.add_parser(
        "run", help="Connect and archive messages until stopped"
    )
    _add_common_opts(run_parser, suppress=True)
    run_parser.set_defaults(func=lambda a: asyncio.run(run(a)))

    replay_parser = sub.add_parser(
        "replay", help="Print archived messages from the NDJSON log"
    )
    _add_common_opts(replay_parser, suppress=True)
    replay_parser.add_argument(
        "--since",
        default=None,
        help="Only replay from this YYYY/MM/basename.ndjson relative path onward (lexical)",
    )
    replay_parser.set_defaults(func=replay)

    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
