use rustc_hash::FxHashMap;
use slotmap::new_key_type;

new_key_type! {
    pub struct SystemKey;
}

pub struct DependencyGraph {
    pub edges: FxHashMap<SystemKey, NodeConnections>,
    pub root: Option<SystemKey>
}

pub struct NodeConnections {
    pub dependents: Vec<SystemKey>,
    pub associates: Vec<SystemKey>
}
impl NodeConnections {
    pub(crate) fn new(dependents: Vec<SystemKey>, associates: Vec<SystemKey>) -> Self {
        NodeConnections {
            dependents,
            associates,
        }
    }
}