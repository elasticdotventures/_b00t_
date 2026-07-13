//! Hook script types and execution.
//!
//! Hook scripts are Rhai expressions stored as string fields in [`BootDatum`]:
//! - `hook_detect`:  runs before version detection
//! - `hook_install`: runs before install
//! - `hook_update`:  runs before update
//! - `hook_learn`:   runs during `b00t learn <topic>`
//! - `hook_uninstall`: runs AFTER uninstall (post-hook, unlike others which are pre-hooks)
//!
//! Hook execution is handled by the [`hook_engine`](crate::hook_engine) module.
//!
//! Return protocol for hooks (via [`hook_engine::HookResult`]):
//! - `Ok` / `Warn(msg)` / `Redirect(datum)` / `Info(msg)` / `Missing`
//!
//! All hook fields on BootDatum are `Option<String>` and are defined in
//! the `boot_datum` module.
