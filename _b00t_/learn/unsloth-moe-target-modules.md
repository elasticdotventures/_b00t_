---
unsloth-moe-target-modules: hardcode target_modules in get_peft_model causes silent config ignore; MoE models with gate_proj/up_proj/down_proj activate 128-expert LoRA = 843M params = 150s/step on A100 — default to attention-only [q_proj,v_proj,k_proj,o_proj] for MoE
