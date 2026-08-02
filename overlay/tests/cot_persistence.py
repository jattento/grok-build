#!/usr/bin/env python3
"""End-to-end check that chain of thought survives across turns.

A reasoning model is only useful across a conversation if what it thought in
turn N is replayed to it in turn N+1. That round trip crosses the gateway, the
wire types and the conversation store, so the only honest way to check it is to
watch the bytes.

The script has two modes and runs both by default.

`stub` is the one that tests this build. A scripted gateway stands in for the
model, so nothing depends on a provider's mood: it emits a marked thought, then
a tool call, then another marked thought, in whichever wire format is under
test. What matters is what grok sends back. It must replay the first thought
inside the same turn (the request that carries the tool result) and both
thoughts on the next turn, after the session has been written to disk and
reloaded by a second process.

`live` runs the same two turns against the real gateway, one model per provider
family, and reports what each provider actually does.

Both modes put a recording proxy in front of the endpoint, point an isolated
GROK_HOME at it, and run two headless turns:

  turn 1  a task that forces the model to think
  turn 2  a follow-up in the same session

Then it asserts, per model:

  emitted   turn 1's response carried reasoning
  replayed  turn 2's request carried that same reasoning back as assistant
            history, non-empty and matching what turn 1 produced
  clean     turn 2 finished without an API or deserialization error

Reasoning travels in one of two forms and both count. Some gateways return the
thought as plaintext (`thinking` / `reasoning_content`); Anthropic subscription
routes return a signed opaque blob instead, and that blob is what the provider
uses to restore the thought on the next turn. The report says which one a model
used.

`recalled` is reported too (turn 2 answered from the earlier thought) but it
depends on the model's willingness to use it, so it never fails the run.

Usage:
    overlay/tests/cot_persistence.py                 # stub + live
    overlay/tests/cot_persistence.py --mode stub     # hermetic, no gateway
    overlay/tests/cot_persistence.py -m claude-opus-5 -m gemini-3.1-flash-lite
    overlay/tests/cot_persistence.py --keep          # keep the recordings
"""

from __future__ import annotations

import argparse
import http.server
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import urllib.error
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GROK_BIN = REPO_ROOT / "target" / "release" / "xai-grok-pager"
UPSTREAM = os.environ.get("COT_UPSTREAM", "http://127.0.0.1:8317")

# The scripted gateway plants these in every thought it emits, so a marker
# found in a later request can only have come from the conversation store.
MARK = "COT-MARK-{:02d}"
STUB_SIGNATURE = "stub-signature-{:02d}"
STUB_FILE = "probe.txt"

# One model per provider family, matching the `cliproxy` catalog in
# ~/.grok/config.toml. Context windows only gate auto-compaction here.
DEFAULT_MODELS = [
    ("claude-opus-5", 1000000),
    ("gpt-5.6-sol", 372000),
    ("gemini-3.1-pro-preview", 1048576),
    ("opencode-kimi-k3", 262144),
    ("copilot-gpt-4.1", 128000),
]

TURN1 = (
    "Un tren sale a las 14:37 y recorre 413 km a 87 km/h, con dos paradas de "
    "12 minutos cada una. Calcula la hora de llegada. En tu respuesta visible "
    "escribi solamente la hora en formato HH:MM, sin explicaciones."
)
TURN2 = (
    "Sin rehacer el calculo: cuantos minutos de viaje puro (sin contar las "
    "paradas) te habian dado? Respondé solo el numero."
)

HOP_BY_HOP = {
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "host",
    "content-length",
}


class Recorder(http.server.ThreadingHTTPServer):
    """Reverse proxy that writes every exchange to `out_dir`."""

    daemon_threads = True
    allow_reuse_address = True

    def __init__(self, out_dir: Path, backend: str = "messages", stub: bool = False,
                 emit_reasoning: bool = True):
        super().__init__(("127.0.0.1", 0), _Handler)
        self.out_dir = out_dir
        self.backend = backend
        self.stub = stub
        self.emit_reasoning = emit_reasoning
        self.counter = 0
        self.thoughts = 0
        self.marks: dict[int, str] = {}
        self.lock = threading.Lock()

    @property
    def base_url(self) -> str:
        host, port = self.server_address[0], self.server_address[1]
        return f"http://{host}:{port}/v1"

    def next_index(self) -> int:
        with self.lock:
            self.counter += 1
            return self.counter

    def next_thought(self) -> int:
        with self.lock:
            self.thoughts += 1
            return self.thoughts


# An agent turn carries grok's whole toolset; the auxiliary calls (titles,
# suggestions) carry one structured-output tool at most.
AGENT_TOOL_COUNT = 5


def sse(event: str, data: dict) -> bytes:
    return f"event: {event}\ndata: {json.dumps(data)}\n\n".encode()


def stub_messages_reply(mark: int, tool_call: bool, reasoning: bool = True) -> bytes:
    """Anthropic Messages: a signed thought, then a tool call or an answer."""
    out = [sse("message_start", {"type": "message_start", "message": {
        "id": f"msg_stub_{mark}", "type": "message", "role": "assistant",
        "model": "stub-model", "content": [], "stop_reason": None,
        "usage": {"input_tokens": 10, "output_tokens": 1}}})]
    if reasoning:
        out.append(sse("content_block_start", {"type": "content_block_start", "index": 0,
                       "content_block": {"type": "thinking", "thinking": "", "signature": ""}}))
        out.append(sse("content_block_delta", {"type": "content_block_delta", "index": 0,
                       "delta": {"type": "thinking_delta",
                                 "thinking": f"{MARK.format(mark)}: pense esto en el paso {mark}."}}))
        out.append(sse("content_block_delta", {"type": "content_block_delta", "index": 0,
                       "delta": {"type": "signature_delta",
                                 "signature": STUB_SIGNATURE.format(mark)}}))
        out.append(sse("content_block_stop", {"type": "content_block_stop", "index": 0}))
    if tool_call:
        out.append(sse("content_block_start", {"type": "content_block_start", "index": 1,
                       "content_block": {"type": "tool_use", "id": f"toolu_stub_{mark}",
                                         "name": "read_file", "input": {}}}))
        out.append(sse("content_block_delta", {"type": "content_block_delta", "index": 1,
                       "delta": {"type": "input_json_delta",
                                 "partial_json": json.dumps({"target_file": STUB_FILE})}}))
        out.append(sse("content_block_stop", {"type": "content_block_stop", "index": 1}))
        stop = "tool_use"
    else:
        out.append(sse("content_block_start", {"type": "content_block_start", "index": 1,
                       "content_block": {"type": "text", "text": ""}}))
        out.append(sse("content_block_delta", {"type": "content_block_delta", "index": 1,
                       "delta": {"type": "text_delta", "text": f"LISTO {MARK.format(mark)}"}}))
        out.append(sse("content_block_stop", {"type": "content_block_stop", "index": 1}))
        stop = "end_turn"
    out.append(sse("message_delta", {"type": "message_delta", "delta": {"stop_reason": stop},
                                     "usage": {"output_tokens": 20}}))
    out.append(sse("message_stop", {"type": "message_stop"}))
    return b"".join(out)


def stub_chat_reply(mark: int, tool_call: bool, reasoning: bool = True) -> bytes:
    """Chat Completions: the same script in `reasoning_content` shape."""
    def chunk(delta: dict, finish=None) -> bytes:
        body = {"id": f"chatcmpl-stub-{mark}", "object": "chat.completion.chunk",
                "created": 0, "model": "stub-model",
                "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]}
        return f"data: {json.dumps(body)}\n\n".encode()

    out = [chunk({"role": "assistant", "content": ""})]
    if reasoning:
        out.append(chunk({"reasoning_content":
                          f"{MARK.format(mark)}: pense esto en el paso {mark}."}))
    if tool_call:
        out.append(chunk({"tool_calls": [{"index": 0, "id": f"call_stub_{mark}",
                                          "type": "function",
                                          "function": {"name": "read_file",
                                                       "arguments": json.dumps(
                                                           {"target_file": STUB_FILE})}}]}))
        out.append(chunk({}, finish="tool_calls"))
    else:
        out.append(chunk({"content": f"LISTO {MARK.format(mark)}"}))
        out.append(chunk({}, finish="stop"))
    out.append(b"data: [DONE]\n\n")
    return b"".join(out)


def stub_plain_reply(backend: str) -> bytes:
    """Answer for the auxiliary calls (titles, suggestions) that carry no tools."""
    if backend == "messages":
        return b"".join([
            sse("message_start", {"type": "message_start", "message": {
                "id": "msg_stub_aux", "type": "message", "role": "assistant",
                "model": "stub-model", "content": [], "stop_reason": None,
                "usage": {"input_tokens": 1, "output_tokens": 1}}}),
            sse("content_block_start", {"type": "content_block_start", "index": 0,
                "content_block": {"type": "text", "text": ""}}),
            sse("content_block_delta", {"type": "content_block_delta", "index": 0,
                "delta": {"type": "text_delta", "text": "Prueba"}}),
            sse("content_block_stop", {"type": "content_block_stop", "index": 0}),
            sse("message_delta", {"type": "message_delta",
                                  "delta": {"stop_reason": "end_turn"},
                                  "usage": {"output_tokens": 2}}),
            sse("message_stop", {"type": "message_stop"}),
        ])
    body = {"id": "chatcmpl-stub-aux", "object": "chat.completion.chunk", "created": 0,
            "model": "stub-model",
            "choices": [{"index": 0, "delta": {"role": "assistant", "content": "Prueba"},
                         "finish_reason": "stop"}]}
    return f"data: {json.dumps(body)}\n\ndata: [DONE]\n\n".encode()


def request_has_tool_result(payload: dict) -> bool:
    for message in payload.get("messages", []):
        if message.get("role") == "tool":
            return True
        content = message.get("content")
        if isinstance(content, list):
            for block in content:
                if isinstance(block, dict) and block.get("type") == "tool_result":
                    return True
    return False

class _Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):  # keep the harness output readable
        pass

    def do_GET(self):
        self._proxy(b"")

    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        self._proxy(self.rfile.read(length))

    def _proxy(self, body: bytes):
        server: Recorder = self.server  # type: ignore[assignment]
        index = server.next_index()
        if body:
            (server.out_dir / f"{index:03d}.request.json").write_bytes(body)

        if server.stub:
            self._serve_stub(server, index, body)
            return

        headers = {
            k: v for k, v in self.headers.items() if k.lower() not in HOP_BY_HOP
        }
        request = urllib.request.Request(
            UPSTREAM + self.path,
            data=body or None,
            headers=headers,
            method=self.command,
        )
        try:
            upstream = urllib.request.urlopen(request)
        except urllib.error.HTTPError as err:
            upstream = err
        except OSError as err:
            self.send_error(502, str(err))
            return

        self.send_response(upstream.status)
        for key, value in upstream.headers.items():
            if key.lower() not in HOP_BY_HOP:
                self.send_header(key, value)
        # No length is known up front, so close the connection to frame the body.
        self.send_header("Connection", "close")
        self.end_headers()

        sink = (server.out_dir / f"{index:03d}.response.txt").open("wb")
        try:
            # Line at a time: SSE is line framed, so this preserves streaming.
            for line in upstream:
                sink.write(line)
                self.wfile.write(line)
                self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass
        finally:
            sink.close()
            upstream.close()
        self.close_connection = True

    def _serve_stub(self, server: Recorder, index: int, body: bytes):
        try:
            payload = json.loads(body) if body else {}
        except ValueError:
            payload = {}

        if not body:  # model listing and other GETs
            reply = json.dumps({"data": [{"id": "stub-model", "object": "model"}]}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(reply)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(reply)
            self.close_connection = True
            return

        if len(payload.get("tools") or []) < AGENT_TOOL_COUNT:
            reply = stub_plain_reply(server.backend)
        else:
            # First call of a turn asks for a tool; the call that brings the
            # tool result back gets a second thought and the final answer.
            tool_call = not request_has_tool_result(payload)
            mark = server.next_thought()
            server.marks[index] = MARK.format(mark)
            reply = (stub_messages_reply(mark, tool_call, server.emit_reasoning)
                     if server.backend == "messages"
                     else stub_chat_reply(mark, tool_call, server.emit_reasoning))

        (server.out_dir / f"{index:03d}.response.txt").write_bytes(reply)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(reply)
        self.wfile.flush()
        self.close_connection = True


def write_config(home: Path, model: str, context_window: int, base_url: str,
                 backend: str = "messages"):
    home.mkdir(parents=True, exist_ok=True)
    # Auxiliary models (titles, suggestions) point at the model under test so
    # nothing in the run depends on a grok.com session.
    (home / "config.toml").write_text(
        f"""[cli]
auto_update = false

[models]
default = "{model}"
default_reasoning_effort = "medium"
max_completion_tokens = 32000
session_summary = "{model}"
prompt_suggestion = "{model}"
image_description = "{model}"

[model_providers.cliproxy]
base_url = "{base_url}"
api_backend = "{backend}"
api_key = "cliproxy-local"

[model."{model}"]
model = "{model}"
model_provider = "cliproxy"
name = "{model}"
context_window = {context_window}
supports_reasoning_effort = true
reasoning_effort = "medium"
reasoning_efforts = ["none", "low", "medium", "high", "xhigh", "max"]
"""
    )


def run_turn(home: Path, cwd: Path, model: str, prompt: str, resume: str | None,
             extra: list[str] | None = None):
    argv = [str(GROK_BIN), "-m", model, "-p", prompt, "--output-format", "json"]
    if resume:
        argv += ["--resume", resume]
    argv += extra or []
    env = dict(os.environ, GROK_HOME=str(home))
    proc = subprocess.run(
        argv, cwd=cwd, env=env, capture_output=True, text=True, timeout=600
    )
    payload = {}
    start = proc.stdout.find("{")
    if start >= 0:
        try:
            payload = json.loads(proc.stdout[start:])
        except json.JSONDecodeError:
            payload = {}
    return proc, payload


class Reasoning:
    """Reasoning material on the wire: plaintext thoughts, signed blobs, or both.

    A turn can hold several thoughts (one before each tool call), so these are
    kept apart instead of concatenated: a replay only has to carry one of them
    back for the chain to be intact, and comparing merged blobs would report
    false failures whenever the model thought more than once.
    """

    def __init__(self):
        self.texts: list[str] = []
        self.signatures: set[str] = set()

    def add_text(self, value):
        if value and value.strip():
            self.texts.append(value.strip())

    def add_signature(self, value):
        if value:
            self.signatures.add(value)

    def extend(self, other: "Reasoning"):
        self.texts.extend(other.texts)
        self.signatures |= other.signatures

    def __bool__(self):
        return bool(self.texts or self.signatures)

    @property
    def kind(self) -> str:
        if self.texts and self.signatures:
            return "plaintext+signed"
        if self.texts:
            return "plaintext"
        return "signed" if self.signatures else "none"

    def matches(self, other: "Reasoning") -> bool:
        """True when `other` carries back at least one of these same thoughts."""
        if self.signatures & other.signatures:
            return True
        replayed = "\n".join(other.texts)
        return any(text[:60] in replayed for text in self.texts if len(text) >= 12)


def reasoning_in_request(path: Path) -> Reasoning:
    """Assistant reasoning replayed to the model, for either wire format."""
    found = Reasoning()
    try:
        payload = json.loads(path.read_text())
    except (ValueError, OSError):
        return found
    for message in payload.get("messages", []):
        if message.get("role") != "assistant":
            continue
        # Anthropic Messages: thinking blocks inside the content array.
        content = message.get("content")
        if isinstance(content, list):
            for block in content:
                if isinstance(block, dict) and block.get("type") == "thinking":
                    found.add_text(block.get("thinking") or "")
                    found.add_signature(block.get("signature") or "")
        # Chat Completions: plain reasoning_content on the message.
        found.add_text(message.get("reasoning_content") or "")
    return found


def reasoning_in_response(path: Path) -> Reasoning:
    """Reasoning the model streamed back, one entry per thinking block."""
    found = Reasoning()
    blocks: dict[int, str] = {}
    chat = ""
    try:
        lines = path.read_text(errors="replace").splitlines()
    except OSError:
        return found
    for line in lines:
        if not line.startswith("data:"):
            continue
        try:
            event = json.loads(line[5:].strip())
        except ValueError:
            continue
        index = event.get("index", 0)
        delta = event.get("delta") or {}
        if delta.get("type") == "thinking_delta":
            blocks[index] = blocks.get(index, "") + (delta.get("thinking") or "")
        if delta.get("type") == "signature_delta":
            found.add_signature(delta.get("signature") or "")
        block = event.get("content_block") or {}
        if block.get("type") == "thinking":
            blocks[index] = blocks.get(index, "") + (block.get("thinking") or "")
            found.add_signature(block.get("signature") or "")
        for choice in event.get("choices") or []:
            chat += (choice.get("delta") or {}).get("reasoning_content") or ""
    for text in blocks.values():
        found.add_text(text)
    found.add_text(chat)
    return found


def stream_error(path: Path) -> str:
    try:
        text = path.read_text(errors="replace")
    except OSError:
        return ""
    for line in text.splitlines():
        if '"error"' in line or '"type":"error"' in line:
            return line[:200]
    return ""


def marks_in_request(path: Path) -> set[str]:
    """Markers the scripted gateway planted, as replayed in assistant history."""
    found: set[str] = set()
    try:
        payload = json.loads(path.read_text())
    except (ValueError, OSError):
        return found
    reasoning = reasoning_in_request(path)
    haystack = "\n".join(reasoning.texts) + "\n" + "\n".join(reasoning.signatures)
    for message in payload.get("messages", []):
        if message.get("role") != "assistant":
            continue
        content = message.get("content")
        if isinstance(content, list):
            for block in content:
                if isinstance(block, dict) and block.get("type") == "thinking":
                    haystack += "\n" + (block.get("thinking") or "")
                    haystack += "\n" + (block.get("signature") or "")
        haystack += "\n" + (message.get("reasoning_content") or "")
    found.update(re.findall(r"COT-MARK-\d\d", haystack))
    found.update(f"COT-MARK-{m}" for m in re.findall(r"stub-signature-(\d\d)", haystack))
    return found


def request_is_agent_turn(path: Path) -> tuple[bool, bool]:
    """(is an agent request, carries a tool result) for one recorded request."""
    try:
        payload = json.loads(path.read_text())
    except (ValueError, OSError):
        return False, False
    return (len(payload.get("tools") or []) >= AGENT_TOOL_COUNT,
            request_has_tool_result(payload))


def check_stub(backend: str, workdir: Path, keep: bool, reasoning: bool = True) -> dict:
    """Drive grok against a scripted gateway and audit what it sends back."""
    name = f"stub/{backend}" + ("" if reasoning else " (control)")
    suffix = backend if reasoning else f"{backend}-control"
    records = workdir / f"records-stub-{suffix}"
    records.mkdir(parents=True, exist_ok=True)
    gateway = Recorder(records, backend=backend, stub=True, emit_reasoning=reasoning)
    threading.Thread(target=gateway.serve_forever, daemon=True).start()

    home = workdir / f"home-stub-{suffix}"
    project = workdir / f"project-stub-{suffix}"
    project.mkdir(parents=True, exist_ok=True)
    (project / STUB_FILE).write_text("banana-42\n")
    write_config(home, "stub-model", 200000, gateway.base_url, backend)

    result = {"model": name, "emitted": False, "replayed": False, "clean": False,
              "recalled": False, "kind": backend, "note": ""}
    allow = ["--permission-mode", "bypassPermissions"]
    try:
        first, payload = run_turn(home, project, "stub-model", TURN1, None, allow)
        session = payload.get("sessionId")
        if not session:
            tail = (first.stderr.strip().splitlines() or ["turn 1 produced no session"])[-1]
            result["note"] = tail[:160]
            return result
        boundary = gateway.counter
        second, _ = run_turn(home, project, "stub-model", TURN2, session, allow)

        requests = sorted(records.glob("*.request.json"))
        turn1 = [p for p in requests if int(p.name[:3]) <= boundary]
        turn2 = [p for p in requests if int(p.name[:3]) > boundary]
        # What the gateway actually thought during turn 1, in order.
        turn1_marks = [mark for index, mark in sorted(gateway.marks.items())
                       if index <= boundary]

        # Inside turn 1: the request that carries the tool result must also
        # carry the thought the model had before calling the tool.
        intra = False
        for path in turn1:
            is_agent, has_result = request_is_agent_turn(path)
            if is_agent and has_result:
                intra = bool(turn1_marks) and turn1_marks[0] in marks_in_request(path)
                break

        # Across turns: a second process resumed the session from disk, so the
        # opening request of turn 2 must carry every thought from turn 1. It
        # also carries turn 1's tool result, so that is not a useful filter.
        cross = False
        for path in turn2:
            is_agent, _ = request_is_agent_turn(path)
            if is_agent:
                marks = marks_in_request(path)
                cross = bool(turn1_marks) and set(turn1_marks) <= marks
                if not cross:
                    result["note"] = (f"turn 1 thought {turn1_marks}, "
                                      f"turn 2 replayed {sorted(marks) or 'nothing'}")
                break

        result["emitted"] = len(turn1_marks) >= 2
        result["replayed"] = bool(intra and cross)
        result["recalled"] = cross
        result["clean"] = first.returncode == 0 and second.returncode == 0
        if not result["clean"]:
            tail = (second.stderr.strip().splitlines() or [""])[-1]
            result["note"] = result["note"] or tail[:160]
        elif intra and not cross:
            result["note"] = result["note"] or "kept within the turn, lost on resume"
        elif cross and not intra:
            result["note"] = "replayed on resume but not inside the turn"
        return result
    finally:
        gateway.shutdown()
        gateway.server_close()
        if not keep:
            shutil.rmtree(records, ignore_errors=True)


def check_model(model: str, context_window: int, workdir: Path, keep: bool) -> dict:
    records = workdir / f"records-{model}"
    records.mkdir(parents=True, exist_ok=True)
    recorder = Recorder(records)
    thread = threading.Thread(target=recorder.serve_forever, daemon=True)
    thread.start()

    home = workdir / f"home-{model}"
    project = workdir / f"project-{model}"
    project.mkdir(parents=True, exist_ok=True)
    write_config(home, model, context_window, recorder.base_url)

    result = {"model": model, "emitted": False, "replayed": False, "clean": False,
              "recalled": False, "kind": "none", "note": ""}
    try:
        first, payload = run_turn(home, project, model, TURN1, None)
        session = payload.get("sessionId")
        if not session:
            result["note"] = (first.stderr.strip().splitlines() or ["turn 1 produced no session"])[-1][:160]
            return result
        boundary = recorder.counter

        second, payload2 = run_turn(home, project, model, TURN2, session)

        requests = sorted(records.glob("*.request.json"))
        responses = sorted(records.glob("*.response.txt"))
        turn1_responses = [p for p in responses if int(p.name[:3]) <= boundary]
        turn2_requests = [p for p in requests if int(p.name[:3]) > boundary]

        emitted = Reasoning()
        for path in turn1_responses:
            emitted.extend(reasoning_in_response(path))
        replayed = Reasoning()
        for path in turn2_requests:
            replayed.extend(reasoning_in_request(path))

        result["emitted"] = bool(emitted)
        result["kind"] = emitted.kind
        result["replayed"] = bool(replayed) and emitted.matches(replayed)
        errors = [stream_error(p) for p in responses if int(p.name[:3]) > boundary]
        errors = [e for e in errors if e]
        result["clean"] = second.returncode == 0 and not errors
        answer = (payload2.get("text") or second.stdout or "").strip()
        result["recalled"] = bool(re.search(r"\b(284|285)\b", answer))
        if not result["clean"]:
            note = errors[0] if errors else (second.stderr.strip().splitlines() or [""])[-1]
            result["note"] = note[:160]
        elif not result["emitted"]:
            result["note"] = "gateway returned no reasoning for this model"
        elif not result["replayed"]:
            result["note"] = "reasoning was produced but not replayed on turn 2"
        return result
    finally:
        recorder.shutdown()
        recorder.server_close()
        if not keep:
            shutil.rmtree(records, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("-m", "--model", action="append", default=[],
                        help="model id to check (repeatable); defaults to one per provider")
    parser.add_argument("--mode", choices=("stub", "live", "both"), default="both",
                        help="stub: scripted gateway (hermetic); live: the real gateway")
    parser.add_argument("--control", action="store_true",
                        help="also run a gateway that emits no reasoning; those runs must fail")
    parser.add_argument("--context-window", type=int, default=200000,
                        help="context window for models passed with -m")
    parser.add_argument("--keep", action="store_true", help="keep the recorded traffic")
    args = parser.parse_args()

    if not GROK_BIN.exists():
        print(f"missing {GROK_BIN}; run `cargo build -p xai-grok-pager-bin --release`",
              file=sys.stderr)
        return 2

    models = ([(m, args.context_window) for m in args.model] or DEFAULT_MODELS)
    workdir = Path(tempfile.mkdtemp(prefix="grok-cot-"))
    print(f"gateway: {UPSTREAM}\nrecordings: {workdir}\n")

    results = []
    if args.mode in ("stub", "both"):
        for backend in ("messages", "chat_completions"):
            print(f"  stub/{backend} ... ", end="", flush=True)
            result = check_stub(backend, workdir, args.keep)
            results.append(result)
            print("emitted" if result["emitted"] else "no-cot", end=" ")
            print("replayed" if result["replayed"] else "not-replayed", end=" ")
            print("clean" if result["clean"] else "errors")

    controls = []
    if args.control:
        for backend in ("messages", "chat_completions"):
            print(f"  stub/{backend} (control) ... ", end="", flush=True)
            control = check_stub(backend, workdir, args.keep, reasoning=False)
            controls.append(control)
            print("replayed (BAD)" if control["replayed"] else "not-replayed (expected)")

    if args.mode in ("live", "both"):
        for model, window in models:
            print(f"  {model} ... ", end="", flush=True)
            result = check_model(model, window, workdir, args.keep)
            results.append(result)
            print("emitted" if result["emitted"] else "no-cot", end=" ")
            print("replayed" if result["replayed"] else "not-replayed", end=" ")
            print("clean" if result["clean"] else "errors")

    print()
    width = max(len(r["model"]) for r in results)
    failures = 0
    for r in results:
        ok = r["emitted"] and r["replayed"] and r["clean"]
        failures += 0 if ok else 1
        flags = (f"emitted={_yn(r['emitted'])} replayed={_yn(r['replayed'])} "
                 f"clean={_yn(r['clean'])} recalled={_yn(r['recalled'])} as={r['kind']}")
        print(f"{'PASS' if ok else 'FAIL'}  {r['model']:<{width}}  {flags}"
              + (f"  :: {r['note']}" if r["note"] else ""))

    if not args.keep:
        shutil.rmtree(workdir, ignore_errors=True)
    print()
    for control in controls:
        if control["replayed"]:
            failures += 1
            print(f"CONTROL LEAK  {control['model']}: reported a replay with no reasoning "
                  "on the wire; the check is not measuring anything")
    print(f"{len(results) - failures}/{len(results)} models replay their chain of thought")
    return 1 if failures else 0


def _yn(value: bool) -> str:
    return "yes" if value else "no"


if __name__ == "__main__":
    sys.exit(main())
