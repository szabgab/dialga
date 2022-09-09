use ahash::AHashMap;
use knurdy::KdlNode;
use smol_str::SmolStr;

use thiserror::Error;

/// Raw instructions for instantiating an entity, as loaded from disc.
#[derive(knuffel::Decode)]
pub struct RawBlueprint {
    #[knuffel(node_name)]
    name: SmolStr,
    #[knuffel(property, str)]
    inherit: Option<SmolStr>,
    #[knuffel(property, default)]
    merge: MergeMode,
    #[knuffel(children)]
    components: Vec<KdlNode>,
}

/// Instructions for instantiating an entity, with all inheritors folded in.
pub struct Blueprint {
    pub name: SmolStr,
    pub components: Vec<KdlNode>,
}

/// A library of all the blueprints.
pub struct BlueprintLibrary {
    /// Map blueprint names to their blueprint.
    prints: AHashMap<SmolStr, RawBlueprint>,
}

impl BlueprintLibrary {
    pub fn new() -> Self {
        Self {
            prints: AHashMap::new(),
        }
    }

    pub fn insert_raw(&mut self, blueprint: RawBlueprint) {
        match self.prints.get_mut(&blueprint.name) {
            None => {
                self.prints.insert(blueprint.name.clone(), blueprint);
            }
            Some(old) => match blueprint.merge {
                MergeMode::Clobber => {
                    *old = blueprint;
                }
                MergeMode::Merge => {
                    for comp in blueprint.components.into_iter() {
                        if let Some(old_comp) = old
                            .components
                            .iter_mut()
                            .find(|old_comp| old_comp.name == comp.name)
                        {
                            *old_comp = comp;
                        } else {
                            old.components.push(comp);
                        }
                    }
                }
            },
        }
    }

    /// Insert all the nodes from the given src string.
    ///
    /// The `filepath` argument is just for error reporting purposes; this doesn't load anything from disc.
    pub fn load_str(&mut self, filepath: &str, src: &str) -> Result<(), knuffel::errors::Error> {
        let raws: Vec<RawBlueprint> = knuffel::parse(filepath, src)?;
        for raw in raws {
            self.insert_raw(raw);
        }

        Ok(())
    }

    /// Attempt to lookup a blueprint in the library and form it into a `KdlNode`.
    pub fn lookup(&self, name: &str) -> Result<Blueprint, BlueprintLookupError> {
        fn recurse(
            lib: &BlueprintLibrary,
            name: &str,
            path: Vec<SmolStr>,
        ) -> Result<Blueprint, BlueprintLookupError> {
            let raw = lib.prints.get(name).ok_or_else(|| match path.as_slice() {
                [] => BlueprintLookupError::NotFound(name.into()),
                [.., last] => BlueprintLookupError::InheriteeNotFound(last.clone(), name.into()),
            })?;
            match &raw.inherit {
                None => Ok(Blueprint {
                    name: raw.name.clone(),
                    components: raw.components.clone(),
                }),
                Some(parent_name) => {
                    if let Some(ono) = path
                        .iter()
                        .enumerate()
                        .find_map(|(idx, kid)| (kid == parent_name).then_some(idx))
                    {
                        let mut problem = path[ono..].to_vec();
                        // Push this parent too to make it clear to the user
                        problem.push(parent_name.clone());
                        return Err(BlueprintLookupError::InheritanceLoop(problem));
                    }

                    let mut path2 = path.clone();
                    path2.push(parent_name.clone());
                    let mut parent = recurse(lib, parent_name, path2)?;

                    for comp in raw.components.iter().cloned() {
                        if let Some(clobberee) = parent
                            .components
                            .iter_mut()
                            .find(|pcomp| pcomp.name == comp.name)
                        {
                            *clobberee = comp;
                        } else {
                            parent.components.push(comp);
                        }
                    }

                    Ok(parent)
                }
            }
        }

        recurse(self, name, Vec::new())
    }
}

/// Problems when looking up a blueprint.
#[derive(Debug, Error)]
pub enum BlueprintLookupError {
    #[error("the blueprint {0} was not found")]
    NotFound(SmolStr),
    #[error("when trying to inherit from another blueprint, the following loop was found: {0:?}")]
    InheritanceLoop(Vec<SmolStr>),
    #[error(
        "the blueprint {0} tried to inherit from the blueprint {1} but the second was not found"
    )]
    InheriteeNotFound(SmolStr, SmolStr),
}

/// How to handle this blueprint if there's another node with the same name.
///
/// When merging blueprints you can only change the old blueprint's components;
/// its inheritor, etc are unchangeable once the blueprint is inserted.
#[derive(Debug, Clone, Copy, Default, knuffel::DecodeScalar)]
pub enum MergeMode {
    /// Merge this node with the old node. This is the default behavior.
    ///
    /// - For components both nodes have, this node's components clobber the old ones.
    /// - For components only this node has, they are all placed after the old nodes.
    /// - Components only the old node has are kept.
    #[default]
    Merge,
    /// Completely replace the old node.
    Clobber,
}
