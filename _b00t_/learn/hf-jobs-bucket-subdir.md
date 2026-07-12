---
HF Jobs bucket volumes (hf: //buckets/...) mounted at /adapters reject mkdir on subdirectories — PermissionError errno 13 on /adapters/subdir even though /adapters root is writable. Use output_dir=/tmp/output and rely on hub_strategy:checkpoint + push_to_hub:true for checkpoint persistence. Confirmed via two failed jobs.
