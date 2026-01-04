import subprocess

dylint_command = [
    "cargo",
    "dylint",
    "--all",
    "--",
    "--no-default-features"
    "--all-targets",
    "--message-format=json",
]
dylint_output = subprocess.run(dylint_command, capture_output=True, text=True)

clippy_command = ["cargo", "clippy", "--all-targets", "--message-format=json", "--no-default-features"]
clippy_output = subprocess.run(clippy_command, capture_output=True, text=True)

combined_output = dylint_output.stdout + clippy_output.stdout

print(combined_output)