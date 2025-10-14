use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use common::peer::NodeAddr;

use crate::ServiceState;

static REGISTRY_ADDR: OnceLock<RwLock<HashMap<String, NodeAddr>>> = OnceLock::new();
static REGISTRY_STATE: OnceLock<RwLock<HashMap<String, Arc<ServiceState>>>> = OnceLock::new();

fn addr_map() -> &'static RwLock<HashMap<String, NodeAddr>> {
    REGISTRY_ADDR.get_or_init(|| RwLock::new(HashMap::new()))
}

fn state_map() -> &'static RwLock<HashMap<String, Arc<ServiceState>>> {
    REGISTRY_STATE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn register(state: Arc<ServiceState>) {
    let pub_hex = state.node().id().to_string();
    // Build a NodeAddr from our node id; iroh will use local routing/relay.
    // We don't have a public socket to add here; NodeAddr is node-id based.
    let addr = NodeAddr::new(*state.node().secret().public());
    addr_map().write().unwrap().insert(pub_hex.clone(), addr);
    state_map().write().unwrap().insert(pub_hex, state);
}

pub fn lookup(public_key_hex: &str) -> Option<NodeAddr> {
    addr_map().read().unwrap().get(public_key_hex).cloned()
}

pub fn lookup_state(public_key_hex: &str) -> Option<Arc<ServiceState>> {
    state_map().read().unwrap().get(public_key_hex).cloned()
}

