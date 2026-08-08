---
console-script-dispatch: python3 -m jupyter nbconvert|lab dispatches to the console script on PATH, which can belong to a DIFFERENT python (~/.local py3.10, broken rpds) and die on import — invoke python3 -m nbconvert / python3 -m jupyter_server directly in the intended env
