//! Session-only quick field presets for RustGIS's sandboxed plugin host.
#[cfg(target_arch = "wasm32")]
mod attribute_values;
#[cfg(target_arch = "wasm32")]
mod column_preferences;
#[cfg(target_arch = "wasm32")]
mod columns;
pub mod edit_history;
#[cfg(target_arch = "wasm32")]
mod expression_filter;
pub mod form_binding;
#[cfg(target_arch = "wasm32")]
mod history_ui;
pub mod model;
pub mod navigation;
pub mod presets;
pub mod previous_view;
#[cfg(target_arch = "wasm32")]
mod publish;
#[cfg(target_arch = "wasm32")]
mod runtime;
#[cfg(target_arch = "wasm32")]
mod settings;
#[cfg(test)]
mod sql_builder;
#[cfg(target_arch = "wasm32")]
mod target;
#[cfg(test)]
mod tests;
#[cfg(target_arch = "wasm32")]
mod ui;
#[cfg(target_arch = "wasm32")]
mod view;
