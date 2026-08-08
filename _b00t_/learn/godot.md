---
platform defensive coding: autoload init ordering varies by export target. Guard all singleton references with get_node_or_null. Null-check both the node AND any properties accessed via the node. Scene _ready() must never crash — every autoload access must be guarded. Fade/scene transitions must use timer timeouts, not bare tween.await.

---
tscn-manifest-crossref: Cross-reference ext_resource Script references in .tscn files against apk_manifest.txt. Missing .gd files cause silent empty scene loading on Android release builds.

---
autoload-instance-vs-static: Call autoload methods via the node reference not the class name. Theme.primary_button() resolves to GDScriptNativeClass static; t.primary_button() where t = get_node('/root/Theme') resolves to the instance.

---
export-pack-fallback: The godot --export-pack command can crash with core dump. Always check that /tmp/godot.pck was actually generated. Without it the APK uses raw assets from a manifest file which may be incomplete.

---
datum-verification: godot was never at /usr/bin — verify paths before writing datums. Extract binary from builder image; 'just screenshot <scene>' gives deploy-free UX PNGs.
