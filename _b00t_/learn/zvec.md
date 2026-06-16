---
zvec: :initialize(None) is NOT idempotent — double-init causes UB. Always gate behind std::sync::OnceLock. Also: dim=0 is a valid u32 parse result but invalid for HNSW — add .filter(|&d| d > 0) to parse_dim.
