FROM pytorch/pytorch:2.5.1-cuda12.4-cudnn9-runtime

WORKDIR /app

# SAM3 runtime deps — transformers for facebook/sam3, supervision for mask RLE
RUN pip install --no-cache-dir \
    transformers>=4.51.0 \
    Pillow \
    numpy \
    supervision \
    requests \
    && pip install --no-cache-dir \
    "huggingface_hub[hf_transfer]"

COPY run.py /app/run.py

# 🤓 Request JSON at /workspace/request.json; output at /workspace/result.json
#    Caller mounts: -v /host/request.json:/workspace/request.json:ro
#                   -v /host/output/:/workspace/:rw
CMD ["python3", "/app/run.py", "/workspace/request.json"]
