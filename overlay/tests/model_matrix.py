#!/usr/bin/env python3
# Disconnected manual harness: not wired to CI or scripts, not a source of
# truth for default models, and must not be treated as coverage.
"""Score one real, multi-tool coding task across every model in the catalog.

A model that answers "OK" proves only that its route is up. This task needs
the whole loop instead: read a file, run a command, read a traceback, edit
code, create two new files, re-run to verify. A model that cannot drive its
tools fails here even though its route is healthy.

The task is executed by *subagents* — that is the path worth testing, since it
is the one real work goes through, and it exercises spawn, the per-subagent
model override, and the result hand-back. Only an agent can spawn a subagent,
so this script does not run the task itself. It owns the two halves that must
be mechanical:

    model_matrix.py prepare        # isolated fixture copy per model
    model_matrix.py grade  ROOT    # verdict per model, from the artifacts

Between the two, the driving agent spawns one subagent per model, each pinned
to that model and pointed at its own directory, in waves of three.

Grading never reads the model's prose, only what it left on disk:

  tests     `pytest` passes, so the fix is real
  report    `python report.py` prints exactly the two lines docs/FORMAT.md asks
  total     total.txt holds the right number
  intact    test_stock.py and inventory.csv are byte-identical to the fixture

`intact` is the one that matters most. The task states the tests are right and
the code is wrong; a model that "passes" by editing the test has not done the
task, and without that check it would score the same as one that did.
"""

from __future__ import annotations

import argparse
import hashlib
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

FIXTURE = Path(__file__).resolve().parent / "fixtures/inventory_task"
REAL_HOME = Path.home() / ".grok"

PROMPT = (
    "Your working directory for this task is {dir}. Read {dir}/README.md and do "
    "everything it asks. Work only inside {dir} — do not read or modify anything "
    "outside it."
)

EXPECTED_REPORT = ["total=490.72", "low=A-100,C-300,E-500"]
EXPECTED_TOTAL = "490.72"
# The task declares these correct. Editing either one is how a model fakes a pass.
IMMUTABLE = ("test_stock.py", "inventory.csv")

CHECKS = ("tests", "report", "total", "intact")


def catalog() -> list[str]:
    """Every model the real config can reach, native model included."""
    cfg = tomllib.loads((REAL_HOME / "config.toml").read_text())
    models = sorted(cfg.get("model", {}))
    # A model with no [model.*] block still resolves, through grok's own
    # provider — that is how the native one is configured, by omission.
    if "grok-4.5" not in models:
        models.append("grok-4.5")
    return models


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


FIXTURE_DIGESTS = {name: digest(FIXTURE / name) for name in IMMUTABLE}


def grade(work: Path) -> tuple[dict[str, bool], str]:
    """Check the artifacts. Never reads what the model said about them."""
    result = dict.fromkeys(CHECKS, False)
    notes = []

    result["intact"] = all(
        (work / name).exists() and digest(work / name) == FIXTURE_DIGESTS[name]
        for name in IMMUTABLE
    )
    if not result["intact"]:
        changed = [n for n in IMMUTABLE
                   if not (work / n).exists()
                   or digest(work / n) != FIXTURE_DIGESTS[n]]
        notes.append("touched " + ",".join(changed))

    tests = subprocess.run([sys.executable, "-m", "pytest", "-q"], cwd=work,
                           capture_output=True, text=True, timeout=120)
    result["tests"] = tests.returncode == 0
    if not result["tests"]:
        tail = [ln for ln in tests.stdout.splitlines() if ln.startswith("FAILED")]
        notes.append(tail[0][:70] if tail else "pytest failed")

    report = work / "report.py"
    if report.exists():
        run = subprocess.run([sys.executable, "report.py"], cwd=work,
                             capture_output=True, text=True, timeout=120)
        lines = run.stdout.strip().splitlines()
        result["report"] = lines == EXPECTED_REPORT
        if not result["report"]:
            notes.append(f"report printed {lines!r}"[:70])
    else:
        notes.append("no report.py")

    total = work / "total.txt"
    if total.exists():
        result["total"] = total.read_text().strip() == EXPECTED_TOTAL
        if not result["total"]:
            notes.append(f"total.txt={total.read_text().strip()[:20]!r}")
    else:
        notes.append("no total.txt")

    return result, "; ".join(notes)


def cmd_prepare(args) -> int:
    root = Path(args.root) if args.root else Path(
        tempfile.mkdtemp(prefix="grok-matrix-"))
    work = root / "work"
    work.mkdir(parents=True, exist_ok=True)
    models = args.model or catalog()
    for model in models:
        target = work / model
        if target.exists():
            shutil.rmtree(target)
        shutil.copytree(FIXTURE, target)
    print(root)
    for model in models:
        print(f"{model}\t{work / model}")
    return 0


def cmd_grade(args) -> int:
    work = Path(args.root) / "work"
    dirs = sorted(p for p in work.iterdir() if p.is_dir())
    if not dirs:
        print(f"no model workdirs under {work}", file=sys.stderr)
        return 2

    results = []
    for d in dirs:
        checks, notes = grade(d)
        results.append({"model": d.name, "checks": checks,
                        "passed": all(checks.values()), "notes": notes})

    results.sort(key=lambda r: (not r["passed"], r["model"]))
    width = max(len(r["model"]) for r in results)
    print(f"{'':<6}{'model':<{width}}  {'  '.join(CHECKS)}")
    for r in results:
        flags = "  ".join(
            f"{'y' if r['checks'][c] else 'n':^{len(c)}}" for c in CHECKS
        )
        print(f"{'PASS' if r['passed'] else 'FAIL':<6}{r['model']:<{width}}  "
              f"{flags}" + (f"  :: {r['notes']}" if r["notes"] else ""))

    passed = sum(r["passed"] for r in results)
    print(f"\n{passed}/{len(results)} models completed the task")
    return 0 if passed == len(results) else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    prep = sub.add_parser("prepare", help="lay down one fixture copy per model")
    prep.add_argument("root", nargs="?", help="where to build; default a temp dir")
    prep.add_argument("-m", "--model", action="append", default=[],
                      help="model to prepare for (repeatable); defaults to the catalog")
    prep.set_defaults(func=cmd_prepare)

    score = sub.add_parser("grade", help="score every workdir under ROOT")
    score.add_argument("root")
    score.set_defaults(func=cmd_grade)

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
