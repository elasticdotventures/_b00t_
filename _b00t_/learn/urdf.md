---
URDF is an XML information artifact for a rooted link/joint tree: each non-root link has one parent joint. Use it for a static articulated rest reference, with meters/radians and explicit joint origin/type/axis/limits. URDF is not an animation container; store motion as BVH or a typed MotionClip sidecar. Never invent inertia, collision, geometry, axes, or limits absent from the source.
