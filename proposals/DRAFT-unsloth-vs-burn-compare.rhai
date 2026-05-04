# Comparison: Unsloth vs Burn for sm0l FOCUS validation training

## Unsloth (Python)

| Pro | Con |
|-----|-----|
| Mature QLoRA support (4-bit) | Requires Python venv, torch, transformers |
| Pre-built recipes for qwen2.5 | Version conflicts with existing b00t .venv |
| 4-bit loading fits 3B in 4GB | Separate training pipeline, not in b00t |
| Large community, many examples | Python ↔ Rust serialization boundary |

## Burn (Rust) — PREFERRED

| Pro | Con |
|-----|-----|
| `Backend` trait — swap CUDA/WGPU/Metal/Vulkan with `type B = Autodiff<Cuda>` | No built-in QLoRA (would need impl) |
| Stays in b00t CLI workspace — no Python | Smaller ecosystem than PyTorch |
| Compile-time type safety for tensor ops | 4-bit quantization not as mature |
| ONNX import for pre-trained weights | Community focused on CNN/transformer examples |
| no_std inference on embedded | Smaller model zoo |
| `burn-train` TUI dashboard | |
| Autodiff decorator pattern (`Autodiff<Wgpu>`) | |

## Recommendation: Burn with Cuda backend

The `Backend` trait pattern aligns perfectly with b00t's design philosophy:

```rust
// Current: hard-coded scores
let roi = sin(seed * 1.0) * 0.4 + 0.5;

// Future: Burn model inference
type Backend = Autodiff<Cuda>;  // train on GPU
type Backend = Autodiff<Wgpu>;  // deploy on Vulkan
type Backend = Autodiff<Flex>;  // deploy on CPU/no_std

let model = FOCUSValidatorConfig::new();
let output = model.forward(input_tensor);
let roi = output[0].into_scalar();
```

But the GTX 1050 (4GB) limits training to 1B models or requires aggressive 4-bit. A `candle`-based approach (already partially in b00t-cli) may be simpler initially.
