---
template-scope-error: Generated JS statements placed inside object literal {} cause browser-only SyntaxError. node -c passes because it validates syntax in isolation but doesn't check structural context. Only CDP/browser console catches these. Fix: move statements before/after the object literal, not inside it.
