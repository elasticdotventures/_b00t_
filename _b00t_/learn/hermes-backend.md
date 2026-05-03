---
environment backend activation: After symlinking a Hermes environment backend into tools/environments/, run hermes config set terminal.backend $NAME to activate. Without this config, the backend is dead code. The backend should print an auto-activation message on first import if terminal.backend isn't set. Disable with HERMES_BACKEND_CHECK=0.
