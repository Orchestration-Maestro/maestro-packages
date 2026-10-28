"""Confirm the async-borrow compiler fault with a disposable local provider."""

import http.server
import json
import subprocess
import sys
import tempfile
import threading
from pathlib import Path
from typing import ClassVar


class Provider(http.server.BaseHTTPRequestHandler):
    """Serve one fixed response without external traffic."""

    requests: ClassVar[list[dict]] = []

    def do_POST(self):
        """Capture each model request and supply a deterministic SSE reply."""
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        self.requests.append(body)
        delta = {"role": "assistant", "content": "Diagnostic provider reply."}
        finish = "stop"
        chunks = [
            {
                "id": "controlled",
                "object": "chat.completion.chunk",
                "model": "guest-test",
                "choices": [{"index": 0, "delta": delta, "finish_reason": None}],
            },
            {
                "id": "controlled",
                "object": "chat.completion.chunk",
                "model": "guest-test",
                "choices": [{"index": 0, "delta": {}, "finish_reason": finish}],
            },
        ]
        payload = (
            "".join("data: " + json.dumps(chunk) + "\n\n" for chunk in chunks)
            + "data: [DONE]\n\n"
        )
        data = payload.encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, format, *args):
        """Keep the verification output limited to assertions and host failures."""


def host_environment(directory, port, package_dir):
    """Write isolated configuration and return an environment without live credentials."""
    agent = Path(directory) / "agent"
    agent.mkdir()
    (agent / "settings.json").write_text(json.dumps({"checkForUpdates": False}))
    (agent / "models.json").write_text(
        json.dumps(
            {
                "providers": {
                    "controlled-guest": {
                        "baseUrl": f"http://127.0.0.1:{port}/v1",
                        "api": "openai-completions",
                        "apiKey": "controlled-placeholder",
                        "models": [
                            {
                                "id": "guest-test",
                                "contextWindow": 128000,
                                "maxTokens": 1024,
                            }
                        ],
                    }
                }
            }
        )
    )
    return {
        "HOME": directory,
        "PATH": "/usr/bin:/bin",
        "MAESTRO_CODING_AGENT_DIR": str(agent),
        "MAESTRO_OFFLINE": "1",
        "MAESTRO_TELEMETRY": "0",
        "MAESTRO_PACKAGE_DIR": package_dir,
    }


def run_host(binary, component, directory, env):
    """Install the local source and submit one input through the real host."""
    installed = subprocess.run(
        [binary, "install", component, "-l"],
        cwd=directory,
        env=env,
        text=True,
        capture_output=True,
        check=False,
        timeout=30,
    )
    assert installed.returncode == 0, installed.stdout + installed.stderr
    result = subprocess.run(
        [
            binary,
            "--offline",
            "--no-session",
            "--no-skills",
            "--no-prompt-templates",
            "--no-context-files",
            "--no-themes",
            "--no-builtin-tools",
            "--provider",
            "controlled-guest",
            "--model",
            "guest-test",
            "--thinking",
            "off",
            "--print",
            "diagnose async borrow",
        ],
        cwd=directory,
        env=env,
        text=True,
        capture_output=True,
        check=False,
        timeout=90,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    return result


def verify(binary, component, package_dir):
    """Succeed only when the pinned toolchain's expected ABI fault is observed."""
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Provider)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        with tempfile.TemporaryDirectory(prefix="maestro-python-") as directory:
            env = host_environment(directory, server.server_port, package_dir)
            result = run_host(binary, component, directory, env)
            assert len(Provider.requests) == 1, (Provider.requests, result.stderr)
            assert "Diagnostic provider reply." in result.stdout, (
                result.stdout + result.stderr
            )
            fault = "borrow handles still remain at the end of the call"
            assert fault in result.stderr, result.stdout + result.stderr
            assert "task.return invoke-event" in result.stderr, result.stderr
            print("CONFIRMED async invoke-event: " + fault)
            print("Host continued after rejecting the component.")
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    verify(*(str(Path(arg).resolve()) for arg in sys.argv[1:4]))
