Yes — the same "run in Docker without installing locally" approach can work for Instructor Embeddings, but with a few important differences.

Here’s a detailed plan for running Instructor Embeddings in a self-contained Docker setup that works offline (e.g., in GitHub Actions) and without GPU.

🛠 Plan: Run instructor-embedding in Docker
1. Understand the Model
The Instructor Embedding models are Hugging Face models like:

hkunlp/instructor-large (768-dim, big, needs ~10–12GB RAM)

hkunlp/instructor-base (512-dim, smaller, ~4–5GB RAM)

hkunlp/instructor-xl (very large, not practical for no-GPU CI)

They are not GGUF (llama.cpp) models — they run via PyTorch (or optionally ONNX).

2. Pick a CI-friendly Version
For GitHub Actions with no GPU, use hkunlp/instructor-base or hkunlp/instructor-small.

Store the model weights locally in the Docker image to avoid network calls.

3. Dockerfile
dockerfile
Copy
Edit
FROM python:3.10-slim

# 1. Install system dependencies
RUN apt-get update && apt-get install -y git && rm -rf /var/lib/apt/lists/*

# 2. Install Python dependencies
RUN pip install --no-cache-dir \
    sentence-transformers \
    InstructorEmbedding \
    torch==2.2.0 --index-url https://download.pytorch.org/whl/cpu

# 3. Pre-download the model so CI has no network calls
RUN python -c "from InstructorEmbedding import INSTRUCTOR; \
    model = INSTRUCTOR('hkunlp/instructor-base')"

# 4. Copy in entrypoint
COPY embed.py /app/embed.py
WORKDIR /app

ENTRYPOINT ["python", "embed.py"]
4. Example embed.py
python
Copy
Edit
from InstructorEmbedding import INSTRUCTOR
import sys

if len(sys.argv) < 2:
    print("Usage: python embed.py 'Your text here'")
    sys.exit(1)

text = sys.argv[1]
model = INSTRUCTOR('hkunlp/instructor-base')
emb = model.encode([["Represent the sentence for retrieval:", text]])
print(emb[0].tolist())
5. Build & Run Locally
bash
Copy
Edit
docker build -t instructor-embed .
docker run --rm instructor-embed "Oreo is a good dog"
6. GitHub Actions Integration
yaml
Copy
Edit
jobs:
  test-embeddings:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Build embedding container
        run: docker build -t instructor-embed .

      - name: Run embedding test
        run: docker run --rm instructor-embed "Oreo is a good dog"
7. Performance Notes
Instructor models are heavier than nomic-embed-text and may be slow in CPU-only CI.

You can quantize via ONNX or optimum for faster runs:

bash
Copy
Edit
pip install optimum onnxruntime
Then export to ONNX and load from disk inside the container.

✅ Bottom line:
Yes — the same self-contained Docker approach works, but instead of llama.cpp you’ll be using PyTorch (or ONNX) for Instructor Embeddings, and you should pre-bake the model weights into the image for offline GitHub Actions runs.

If you want, I can make you a minimal CPU-only ONNX-quantized Instructor Embedding Dockerfile so your GitHub Actions run 5–10× faster. That would make CI much smoother.

