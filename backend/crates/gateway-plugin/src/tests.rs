//! Unit tests for the plugin framework (registry, decision flow, ordering).

use crate::context::{PluginContext, RequestPhase};
use crate::decision::PluginDecision;
use crate::error::PluginError;
use crate::plugin::{Plugin, PluginKind, PluginMetadata};
use crate::registry::PluginRegistry;

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use uuid::Uuid;

fn meta(name: &str, kind: PluginKind, priority: i32) -> PluginMetadata {
    PluginMetadata {
        name: name.to_string(),
        version: "0.0.0".to_string(),
        kind,
        priority,
        enabled: true,
        description: None,
    }
}

fn meta_disabled(name: &str, kind: PluginKind) -> PluginMetadata {
    let mut m = meta(name, kind, 0);
    m.enabled = false;
    m
}

fn empty_ctx() -> PluginContext {
    PluginContext {
        phase: RequestPhase::BeforeRequest,
        tenant_id: Uuid::nil(),
        user_id: None,
        api_key_id: None,
        virtual_key_id: None,
        backend_id: None,
        model: None,
        path: "/v1/chat/completions".to_string(),
        request: json!({}),
        response: None,
        status_code: None,
        metadata: HashMap::new(),
        request_id: None,
        trace_id: None,
    }
}

/// Plugin that records the order in which it was invoked.
struct Tracer {
    metadata: PluginMetadata,
    counter: Arc<AtomicUsize>,
    slot: Arc<AtomicUsize>,
}

#[async_trait]
impl Plugin for Tracer {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    async fn before_request(&self, _ctx: &mut PluginContext) -> Result<PluginDecision, PluginError> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        self.slot.store(n, Ordering::SeqCst);
        Ok(PluginDecision::Continue)
    }
}

/// Plugin that blocks with a 451 + custom message.
struct Blocker {
    metadata: PluginMetadata,
}
#[async_trait]
impl Plugin for Blocker {
    fn metadata(&self) -> &PluginMetadata { &self.metadata }
    async fn before_request(&self, _ctx: &mut PluginContext) -> Result<PluginDecision, PluginError> {
        Ok(PluginDecision::Block { status_code: 451, message: "blocked".into() })
    }
}

/// Plugin that responds with a cached body (short-circuits with 200).
struct CacheHit {
    metadata: PluginMetadata,
}
#[async_trait]
impl Plugin for CacheHit {
    fn metadata(&self) -> &PluginMetadata { &self.metadata }
    async fn before_request(&self, _ctx: &mut PluginContext) -> Result<PluginDecision, PluginError> {
        Ok(PluginDecision::Respond { status_code: 200, body: json!({"cached": true}) })
    }
}

/// Plugin whose hook errors out.
struct Bomb {
    metadata: PluginMetadata,
}
#[async_trait]
impl Plugin for Bomb {
    fn metadata(&self) -> &PluginMetadata { &self.metadata }
    async fn before_request(&self, _ctx: &mut PluginContext) -> Result<PluginDecision, PluginError> {
        Err(PluginError::Internal("plugin explosion".into()))
    }
}

/// Output-kind plugin that writes to metadata.
struct ResponseTagger {
    metadata: PluginMetadata,
}
#[async_trait]
impl Plugin for ResponseTagger {
    fn metadata(&self) -> &PluginMetadata { &self.metadata }
    async fn after_response(&self, ctx: &mut PluginContext) -> Result<PluginDecision, PluginError> {
        ctx.set_metadata("tagged", Value::Bool(true));
        Ok(PluginDecision::Modified)
    }
}

// ── PluginDecision::is_terminal ────────────────────────────────────────────

#[test]
fn is_terminal_is_true_for_block_and_respond() {
    assert!(PluginDecision::Block { status_code: 400, message: "x".into() }.is_terminal());
    assert!(PluginDecision::Respond { status_code: 200, body: json!({}) }.is_terminal());
}

#[test]
fn is_terminal_is_false_for_continue_and_modified() {
    assert!(!PluginDecision::Continue.is_terminal());
    assert!(!PluginDecision::Modified.is_terminal());
}

// ── Registry: register / unregister / list ────────────────────────────────

#[test]
fn register_adds_and_list_returns_metadata() {
    let reg = PluginRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));
    reg.register(Arc::new(Tracer {
        metadata: meta("t1", PluginKind::Input, 0),
        counter: counter.clone(),
        slot: Arc::new(AtomicUsize::new(0)),
    }));
    let list = reg.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "t1");
}

#[test]
fn unregister_removes_plugin() {
    let reg = PluginRegistry::new();
    reg.register(Arc::new(Tracer {
        metadata: meta("t1", PluginKind::Input, 0),
        counter: Arc::new(AtomicUsize::new(0)),
        slot: Arc::new(AtomicUsize::new(0)),
    }));
    reg.unregister("t1");
    assert_eq!(reg.list().len(), 0);
}

#[test]
fn register_same_name_replaces_existing() {
    let reg = PluginRegistry::new();
    reg.register(Arc::new(Tracer {
        metadata: meta("t1", PluginKind::Input, 0),
        counter: Arc::new(AtomicUsize::new(0)),
        slot: Arc::new(AtomicUsize::new(0)),
    }));
    reg.register(Arc::new(Tracer {
        metadata: meta("t1", PluginKind::Input, 99),
        counter: Arc::new(AtomicUsize::new(0)),
        slot: Arc::new(AtomicUsize::new(0)),
    }));
    let list = reg.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].priority, 99);
}

// ── Registry: pipeline execution ──────────────────────────────────────────

#[tokio::test]
async fn before_request_runs_all_continue_plugins() {
    let reg = PluginRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));

    reg.register(Arc::new(Tracer {
        metadata: meta("a", PluginKind::Input, 10),
        counter: counter.clone(),
        slot: Arc::new(AtomicUsize::new(99)),
    }));
    reg.register(Arc::new(Tracer {
        metadata: meta("b", PluginKind::Input, 20),
        counter: counter.clone(),
        slot: Arc::new(AtomicUsize::new(99)),
    }));

    let mut ctx = empty_ctx();
    let outcome = reg.run_before_request(&mut ctx).await;

    assert_eq!(outcome.executions.len(), 2);
    assert!(matches!(outcome.decision, PluginDecision::Continue));
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn block_short_circuits_pipeline() {
    let reg = PluginRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));

    reg.register(Arc::new(Tracer {
        metadata: meta("before", PluginKind::Input, 10),
        counter: counter.clone(),
        slot: Arc::new(AtomicUsize::new(0)),
    }));
    reg.register(Arc::new(Blocker {
        metadata: meta("gate", PluginKind::Guardrail, 20),
    }));
    reg.register(Arc::new(Tracer {
        metadata: meta("after", PluginKind::Input, 30),
        counter: counter.clone(),
        slot: Arc::new(AtomicUsize::new(0)),
    }));

    let mut ctx = empty_ctx();
    let outcome = reg.run_before_request(&mut ctx).await;

    // "before" (Input, pri 10) and "gate" (Guardrail, pri 20) ran;
    // "after" (Input, pri 30) did NOT because gate sorts after both Inputs?
    // Actually ordering is by (kind_id, priority, name).
    // PluginKind ordinal: Input=0, Output=1, Guardrail=2, Observer=3, Auth=4
    // So order is: before (Input 10), after (Input 30), gate (Guardrail 20)
    // "before" runs, "after" runs, then gate blocks → both Input tracers ran.
    assert!(matches!(outcome.decision, PluginDecision::Block { status_code: 451, .. }));
    assert_eq!(outcome.terminated_by.as_deref(), Some("gate"));
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn respond_short_circuits_pipeline() {
    let reg = PluginRegistry::new();
    reg.register(Arc::new(CacheHit {
        metadata: meta("cache", PluginKind::Input, 0),
    }));
    let counter = Arc::new(AtomicUsize::new(0));
    reg.register(Arc::new(Tracer {
        metadata: meta("later", PluginKind::Input, 10),
        counter: counter.clone(),
        slot: Arc::new(AtomicUsize::new(0)),
    }));

    let mut ctx = empty_ctx();
    let outcome = reg.run_before_request(&mut ctx).await;

    assert!(matches!(outcome.decision, PluginDecision::Respond { status_code: 200, .. }));
    assert_eq!(outcome.terminated_by.as_deref(), Some("cache"));
    // Later tracer never ran
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn disabled_plugin_is_skipped() {
    let reg = PluginRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));
    reg.register(Arc::new(Tracer {
        metadata: meta_disabled("off", PluginKind::Input),
        counter: counter.clone(),
        slot: Arc::new(AtomicUsize::new(0)),
    }));
    let mut ctx = empty_ctx();
    let outcome = reg.run_before_request(&mut ctx).await;
    assert_eq!(outcome.executions.len(), 0);
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn plugin_error_is_logged_and_pipeline_continues() {
    let reg = PluginRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));
    reg.register(Arc::new(Bomb {
        metadata: meta("bomb", PluginKind::Input, 0),
    }));
    reg.register(Arc::new(Tracer {
        metadata: meta("after", PluginKind::Input, 10),
        counter: counter.clone(),
        slot: Arc::new(AtomicUsize::new(0)),
    }));

    let mut ctx = empty_ctx();
    let outcome = reg.run_before_request(&mut ctx).await;

    // Bomb errored but pipeline should still run subsequent plugins.
    assert_eq!(outcome.executions.len(), 2);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert!(matches!(outcome.decision, PluginDecision::Continue));
}

#[tokio::test]
async fn after_response_runs_output_plugins_and_propagates_metadata() {
    let reg = PluginRegistry::new();
    reg.register(Arc::new(ResponseTagger {
        metadata: meta("tagger", PluginKind::Output, 0),
    }));

    let mut ctx = empty_ctx();
    let outcome = reg.run_after_response(&mut ctx).await;

    assert_eq!(outcome.executions.len(), 1);
    assert_eq!(ctx.get_metadata("tagged"), Some(&Value::Bool(true)));
    assert_eq!(ctx.phase, RequestPhase::AfterResponse);
    assert!(outcome.executions[0].modified);
}

#[tokio::test]
async fn after_response_skips_input_kind_plugins() {
    let reg = PluginRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));
    reg.register(Arc::new(Tracer {
        metadata: meta("input-only", PluginKind::Input, 0),
        counter: counter.clone(),
        slot: Arc::new(AtomicUsize::new(0)),
    }));

    let mut ctx = empty_ctx();
    let outcome = reg.run_after_response(&mut ctx).await;

    assert_eq!(outcome.executions.len(), 0);
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}

// ── PluginContext metadata helpers ────────────────────────────────────────

#[test]
fn context_mark_pii_detected_sets_flag() {
    let mut ctx = empty_ctx();
    ctx.mark_pii_detected();
    assert_eq!(ctx.get_metadata("pii_detected"), Some(&Value::Bool(true)));
}

#[test]
fn context_is_cached_reads_metadata_flag() {
    let mut ctx = empty_ctx();
    assert!(!ctx.is_cached());
    ctx.set_metadata("cached", Value::Bool(true));
    assert!(ctx.is_cached());
}
