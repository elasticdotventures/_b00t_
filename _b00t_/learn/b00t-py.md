---
native module naming: PyO3 .so files in Python packages must match the module name in #[pymodule]. A package named b00t_py containing b00t_py.cpython-*.so creates a circular import when __init__.py does from b00t_py._core import. Fix: the native module is imported as b00t_py.b00t_py, not b00t_py._core. Use lazy loading via importlib.import_module() to avoid circular import at module level.
