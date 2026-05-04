// b00t model training — design proposal
// Model serving AND fine-tuning are both first-class b00t features.
//
// Serving:  b00t model serve <model>          — start inference endpoint
// Training: b00t model train <base> --data <examples.jsonl> — LoRA adapt
// Config:   _b00t_/<model>.training.tomllmd   — training datum
//
// Principle: use existing infrastructure. No new DL framework dependency.
// The llamacpp binary already handles both inference AND LoRA fine-tuning.

// ─── Architecture ────────────────────────────────────────────────────────────

// b00t model train --base qwen2.5-3b --data .b00t/fsl/focus-examples.jsonl
//   │
//   ├── 1. Load training datum (model.training.tomllmd)
//   │      hyperparameters, dataset, output adapter path
//   │
//   ├── 2. Convert FSL examples to training format
//   │      .b00t/fsl/ → .b00t/models/adapters/<name>/train.jsonl
//   │
//   ├── 3. Spawn llamacpp finetune subprocess
//   │      llama-finetune --model <base> --data <train> --lora <out>
//   │      (llamacpp supports LoRA on CPU — no GPU needed)
//   │
//   └── 4. Register adapter as b00t datum
//         _b00t_/<name>.adapter.tomllmd → loadable by serving endpoint

// ─── Training datum (_b00t_/focus-validator.training.tomllmd) ───────────────

// [b00t]
// name = "focus-validator"
// type = "training"
// base_model = "qwen2.5-1.5b"
// adapter_output = ".b00t/models/adapters/focus-validator/"
//
// [b00t.training.data]
// source = ".b00t/fsl/focus-examples.jsonl"
// format = "t00n-fsl"  # t00n-serialized (input, expected_output)
// max_examples = 500
// validation_split = 0.1
//
// [b00t.training.hyperparameters]
// learning_rate = 1e-4
// num_epochs = 3
// batch_size = 1       # CPU training, keep it small
// max_seq_length = 512
// lora_r = 8
// lora_alpha = 16
// lora_dropout = 0.05
//
// [b00t.training.backend]
// # Use existing llamacpp binary for LoRA (no Python, no candle build issues)
// method = "llamacpp-finetune"
// # Future: candle-lora (when candle deps resolve)
// # Future: burn-lora (when burn crate is evaluated)

// ─── Commands to add ─────────────────────────────────────────────────────────

// b00t model create    — scaffold training datum from an existing model
// b00t model train     — run LoRA fine-tuning using llamacpp
// b00t model serve     — start serving with adapter loaded (already exists)
// b00t model ls        — list available models + adapters
