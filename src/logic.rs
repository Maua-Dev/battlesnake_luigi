//! Lambda adapter for the same SnakeBollada DecisionState/FutureGraph used
//! by the native server. The Lambda handler and Terraform remain independent.
use crate::models::GameState as RequestGameState;
use crate::runtime::GameRuntime;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::Instant;

static RUNTIME: OnceLock<GameRuntime> = OnceLock::new();
fn runtime() -> &'static GameRuntime {
    RUNTIME.get_or_init(GameRuntime::new)
}

pub fn info() -> Value {
    json!({
        "apiversion": "1",
        "author": "",
        "color": "#8B0000",
        "head": "tiger-king",
        "tail": "hook",
        "version": "ordering-v3-lambda"
    })
}
pub async fn start(input: &RequestGameState) {
    runtime().start(&crate::GameState::from(input)).await;
}
pub async fn get_move(input: &RequestGameState) -> Value {
    // Include conversion work in the request-side search deadline.
    let started = Instant::now();
    let state = crate::GameState::from(input);
    let decision = runtime().decide(&state, started).await;
    json!({"move": decision.direction.as_str()})
}
pub async fn end(input: &RequestGameState) {
    runtime().end(&crate::GameState::from(input)).await;
}
