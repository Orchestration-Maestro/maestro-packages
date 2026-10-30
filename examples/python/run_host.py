"""Run the Python extension through a real Maestro host with a disposable local provider."""

import http.server
import json
import subprocess
import sys
import tempfile
import threading
from pathlib import Path
from typing import ClassVar


class Provider(http.server.BaseHTTPRequestHandler):
    """Serve a tool call to `hello`, then a final reply, without external traffic."""

    requests: ClassVar[list[dict]] = []

    def do_POST(self):
        """Capture each model request and supply a deterministic SSE reply."""
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        self.requests.append(body)
        if len(self.requests) == 1:
            call = {
                "index": 0,
                "id": "call_1",
                "type": "function",
                "function": {"name": "hello", "arguments": json.dumps({"name": "Ada"})},
            }
            delta = {"role": "assistant", "tool_calls": [call]}
            finish = "tool_calls"
        else:
            delta = {"role": "assistant", "content": "Greeting delivered."}
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
            "greet Ada",
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


def message_text(message):
    """Join the text parts of one chat message, whether plain or a list of parts."""
    content = message["content"]
    if isinstance(content, str):
        return content
    return "".join(part.get("text", "") for part in content)


def verify(binary, component, package_dir):
    """Succeed only when the input hook and the tool both ran inside the host."""
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Provider)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        with tempfile.TemporaryDirectory(prefix="maestro-python-") as directory:
            env = host_environment(directory, server.server_port, package_dir)
            result = run_host(binary, component, directory, env)
            assert "component fault" not in result.stderr, result.stderr
            assert len(Provider.requests) == 2, (Provider.requests, result.stderr)
            first, second = (
                [message_text(m) for m in request["messages"] if m["role"] == role]
                for request, role in zip(Provider.requests, ("user", "tool"), strict=True)
            )
            assert first[-1] == "GREET ADA", first
            assert second == ["Hello, Ada!"], second
            assert "Greeting delivered." in result.stdout, result.stdout + result.stderr
            print("input hook: the model received " + repr(first[-1]))
            print("hello tool: the model received " + repr(second[0]))
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    verify(*(str(Path(arg).resolve()) for arg in sys.argv[1:4]))
