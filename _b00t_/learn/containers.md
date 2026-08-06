---
uv pip rejects legacy wheel metadata common in CUDA/ML stacks: nvdiffrast-style setup.py wheels (Metadata field Name not found) and pytorch cu124 index nvidia wheels (Invalid Wheel-Version: None). Inside Containerfile builds for torch+CUDA-extension stacks use plain python3 -m pip; keep uv for host workflows and clean modern deps.

---
CORRECTION (operator directive): never fall back to pip even inside Containerfiles — always uv. When uv rejects legacy wheel metadata (pytorch cu124 index 'Invalid Wheel-Version: None'; nvdiffrast-style sdist 'Metadata field Name not found'): (1) base the image on pytorch/pytorch:*-devel so torch arrives as shared registry layers instead of an install step, (2) install uv via COPY --from=ghcr.io/astral-sh/uv:latest, (3) uv pip install --system setuptools wheel ninja before any --no-build-isolation source builds.

---
CORRECTION (operator directive): never fall back to pip even inside Containerfiles — always uv. When uv rejects legacy wheel metadata: (1) base image = pytorch/pytorch:*-devel so torch arrives as shared registry layers, (2) uv via COPY --from=ghcr.io/astral-sh/uv:latest, (3) uv pip install --system setuptools wheel ninja before --no-build-isolation source builds.
