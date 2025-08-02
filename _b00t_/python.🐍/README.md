## b00t syntax style guide 🥾💌🐍

🤓: laconic docstring comments, idiomatic justfiles for commands 
(it should not be necessary to read python to understand the abstract capabilities of a
repo)


- ALWAYS ruff, NEVER black, or other lints.
- ALWAYS PEP484 (Type Hints) https://peps.python.org/pep-0484/
- ALWAYS using uv, uvx (NEVER poetry or pip!), PREFER pixi not conda
	* unless project already uses conda
	* avoid editing pyproject.toml directly; use uv! 
- 

- MCP
	* https://github.com/jlowin/fastmcp >= 0.4.0

- prefer FastAPI,
	* Error Handling:
	- DRY PYTHON "returns" module to emulate Rust Option, Some, Nothing https://github.com/dry-python/returns

```
from returns.result import Result, Success, Failure
from returns.option import Option, Some, Nothing

def get_user(id: int) -> Result[str, Exception]:
    if id == 1:
        return Success("Alice")
    else:
        return Failure(ValueError("Not found"))

match get_user(2):
    case Success(user):
        print(user)
    case Failure(error):
        print(f"Oops: {error}")
```
	 - PEP 654 __cause__, __context__, or rich tracebacks (grouped exceptions native in py 3.11)
	`raise ExceptionGroup("Multiple failures", [IOError("disk"), ValueError("bad input")])`
	- Use chained exceptions (raise X from Y)
```

```
	- __str__, __repr__, plus logging + traceback module
	- Exception hierarchy + decorators/middleware



