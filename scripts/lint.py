import subprocess
import os
import tempfile

# Set CARGO_TARGET_DIR to TEMP to avoid Windows path length issues with dylint
env = os.environ.copy()
env["CARGO_TARGET_DIR"] = os.path.join(tempfile.gettempdir(), "dylint")

dylint_command = [
    "cargo",
    "dylint",
    "--all",
    "--",
    "--no-default-features"
    "--all-targets",
    "--message-format=json",
]
dylint_output = subprocess.run(dylint_command, capture_output=True, text=True, env=env)

clippy_command = ["cargo", "clippy", "--all-targets", "--message-format=json", "--no-default-features"]
clippy_output = subprocess.run(clippy_command, capture_output=True, text=True)

combined_output = dylint_output.stdout + clippy_output.stdout

print(combined_output)