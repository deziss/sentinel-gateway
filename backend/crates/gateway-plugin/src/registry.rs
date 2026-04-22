use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;

use crate::context::{PluginContext, RequestPhase};
use crate::decision::{PluginDecision, PluginExecution, PluginOutcome};
use crate::plugin::{Plugin, PluginKind};

/// Thread-safe ordered plugin registry.
///
/// Plugins are stored grouped by `PluginKind`, sorted by priority ascending.
/// The pipeline invokes all plugins of a kind in order until one returns a
/// terminal decision (`Block` or `Respond`).
///
/// Plugins can be hot-swapped at runtime via `register` / `unregister`.
pub struct PluginRegistry {
    plugins: RwLock<Vec<Arc<dyn Plugin>>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(Vec::new()),
        }
    }

    /// Register a plugin. Re-registration (same name) replaces the existing one.
    /// Plugins are resorted by (kind, priority, name) after registration.
    pub fn register(&self, plugin: Arc<dyn Plugin>) {
        let mut guard = self.plugins.write();
        let name = plugin.name().to_string();
        guard.retain(|p| p.name() != name);
        guard.push(plugin);
        guard.sort_by(|a, b| {
            let ka = a.metadata();
            let kb = b.metadata();
            (ka.kind as u8, ka.priority, ka.name.clone())
                .cmp(&(kb.kind as u8, kb.priority, kb.name.clone()))
        });
    }

    /// Remove a plugin by name.
    pub fn unregister(&self, name: &str) {
        self.plugins.write().retain(|p| p.name() != name);
    }

    /// Enable/disable a plugin by name (keeps it registered).
    /// Returns false if the plugin is not found.
    pub fn set_enabled(&self, name: &str, enabled: bool) -> bool {
        // Plugins don't hold mutable state here; the enabled flag is baked into
        // metadata() which comes from the plugin itself. For dynamic toggling
        // plugins should implement their own reload mechanism. This helper is
        // left as a marker — the caller re-registers the plugin with updated
        // config.
        let _ = (name, enabled);
        false
    }

    /// List all registered plugins with metadata.
    pub fn list(&self) -> Vec<crate::plugin::PluginMetadata> {
        self.plugins.read().iter().map(|p| p.metadata().clone()).collect()
    }

    /// Execute the `before_request` pipeline for all Input + Guardrail plugins.
    /// Observer plugins run concurrently via `on_request` (no blocking).
    pub async fn run_before_request(&self, ctx: &mut PluginContext) -> PluginOutcome {
        self.run_phase(ctx, RequestPhase::BeforeRequest).await
    }

    /// Execute the `after_response` pipeline for Output + Observer plugins.
    pub async fn run_after_response(&self, ctx: &mut PluginContext) -> PluginOutcome {
        self.run_phase(ctx, RequestPhase::AfterResponse).await
    }

    async fn run_phase(&self, ctx: &mut PluginContext, phase: RequestPhase) -> PluginOutcome {
        ctx.phase = phase;
        let snapshot: Vec<Arc<dyn Plugin>> = self.plugins.read().clone();
        let mut executions = Vec::new();
        let mut final_decision = PluginDecision::Continue;
        let mut terminated_by: Option<String> = None;

        for plugin in snapshot.iter() {
            if !plugin.metadata().enabled {
                continue;
            }

            // Only run relevant plugins for the current phase
            let runs_here = matches!(
                (phase, plugin.kind()),
                (RequestPhase::BeforeRequest, PluginKind::Input)
                    | (RequestPhase::BeforeRequest, PluginKind::Guardrail)
                    | (RequestPhase::BeforeRequest, PluginKind::Auth)
                    | (RequestPhase::AfterResponse, PluginKind::Output)
                    | (RequestPhase::AfterResponse, PluginKind::Observer)
            );
            if !runs_here {
                continue;
            }

            let start = Instant::now();
            let result = match phase {
                RequestPhase::BeforeRequest => plugin.before_request(ctx).await,
                RequestPhase::AfterResponse => plugin.after_response(ctx).await,
                RequestPhase::OnError => Ok(PluginDecision::Continue),
            };
            let elapsed = start.elapsed().as_millis() as u64;

            match result {
                Ok(decision) => {
                    let modified = matches!(decision, PluginDecision::Modified);
                    executions.push(PluginExecution {
                        plugin_name: plugin.name().to_string(),
                        duration_ms: elapsed,
                        modified,
                    });

                    if decision.is_terminal() {
                        terminated_by = Some(plugin.name().to_string());
                        final_decision = decision;
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        plugin = plugin.name(),
                        error = %e,
                        "Plugin execution failed — skipping"
                    );
                    executions.push(PluginExecution {
                        plugin_name: plugin.name().to_string(),
                        duration_ms: elapsed,
                        modified: false,
                    });
                }
            }
        }

        PluginOutcome {
            decision: final_decision,
            terminated_by,
            executions,
        }
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
