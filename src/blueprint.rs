use ahash::AHashMap;
use knurdy::KdlNode;
use smol_str::SmolStr;

/// Instructions for instantiating an entity.
pub struct Blueprint {
    name: SmolStr,
    inherit: Option<SmolStr>,
    merge: MergeMode,
    components: Vec<KdlNode>,
}

/// A library of all the blueprints.
pub struct BlueprintLibrary {
    /// Map blueprint names to their blueprint.
    prints: AHashMap<SmolStr, Blueprint>,
}

impl BlueprintLibrary {
    pub fn load(src: &str) -> Result<Self, >
}

/// How to handle this blueprint if there's another node with the same name.
#[derive(Debug, Clone, Copy)]
pub enum MergeMode {
    /// Completely replace the old node.
    Clobber,
    /// Merge this node with the old node.
    ///
    /// - For components both nodes have, this node's components clobber the old ones.
    /// - For components only this node has, they are all placed after the old nodes.
    /// - Components only the old node has are kept.
    Merge,
}
