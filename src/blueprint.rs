use std::fmt::Display;

use ahash::AHashMap;
use kdl::{KdlDocument, KdlNode};
use miette::{Diagnostic, LabeledSpan, NamedSource, Severity, SourceCode, SourceSpan};
use smol_str::SmolStr;

use thiserror::Error;

/// Raw instructions for instantiating an entity, as loaded from disc.
pub struct RawBlueprint {
    name: SmolStr,
    inherit: Option<SmolStr>,
    merge: MergeMode,
    components: Vec<KdlNode>,
}

impl RawBlueprint {
    pub fn load_from_kdl(
        doc: &KdlDocument,
        src: NamedSource,
    ) -> Result<Vec<RawBlueprint>, RawBlueprintDeserError> {
        let mut out = Vec::new();
        for kid in doc.nodes() {
            let comps = match kid.children() {
                Some(comps) => comps,
                None => {
                    return Err(RawBlueprintDeserError {
                        span: *kid.span(),
                        kind: RawBlueprintParseErrorKind::NoChildren,
                        src,
                    })
                }
            };

            let mut inherit = None;
            let mut merge = None;
            for entry in kid.entries() {
                let key = if let Some(key) = entry.name() {
                    key
                } else {
                    return Err(RawBlueprintDeserError {
                        span: *entry.span(),
                        kind: RawBlueprintParseErrorKind::TopLevelArgument,
                        src,
                    });
                };

                match key.value() {
                    "inherit" => {
                        if inherit.is_some() {
                            return Err(RawBlueprintDeserError {
                                span: *entry.span(),
                                kind: RawBlueprintParseErrorKind::ClobberInherit,
                                src,
                            });
                        }

                        if let Some(string) = entry.value().as_string() {
                            inherit = Some(string.into());
                        } else {
                            return Err(RawBlueprintDeserError {
                                span: *entry.span(),
                                kind: RawBlueprintParseErrorKind::NonStringInherit,
                                src,
                            });
                        }
                    }
                    "merge" => {
                        if merge.is_some() {
                            return Err(RawBlueprintDeserError {
                                span: *entry.span(),
                                kind: RawBlueprintParseErrorKind::ClobberInherit,
                                src,
                            });
                        }

                        let mode = if let Some(s) = entry.value().as_string() {
                            s
                        } else {
                            return Err(RawBlueprintDeserError {
                                span: *entry.span(),
                                kind: RawBlueprintParseErrorKind::BadMerge,
                                src,
                            });
                        };
                        let mode = match mode.to_lowercase().as_str() {
                            "merge" => MergeMode::Merge,
                            "clobber" => MergeMode::Clobber,
                            _ => {
                                return Err(RawBlueprintDeserError {
                                    span: *entry.span(),
                                    kind: RawBlueprintParseErrorKind::BadMerge,
                                    src,
                                })
                            }
                        };
                        merge = Some(mode);
                    }
                    _ => {
                        return Err(RawBlueprintDeserError {
                            span: *entry.span(),
                            kind: RawBlueprintParseErrorKind::InvalidKey,
                            src,
                        })
                    }
                }

                // We check down here because it's the "least important" error
                if entry.ty().is_some() {
                    return Err(RawBlueprintDeserError {
                        span: *entry.span(),
                        kind: RawBlueprintParseErrorKind::TopLevelAnnotation,
                        src,
                    });
                }
            }

            let merge = merge.unwrap_or_default();
            let components = comps.nodes().iter().cloned().collect();

            let bp = RawBlueprint {
                name: kid.name().value().into(),
                inherit,
                merge,
                components,
            };
            out.push(bp)
        }

        Ok(out)
    }
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
                            .find(|old_comp| old_comp.name() == comp.name())
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
    pub fn load_str(&mut self, src: &str, filename: &str) -> Result<(), BlueprintParseError> {
        let doc = src.parse()?;
        let source = NamedSource::new(filename, src.to_owned());
        let raws = RawBlueprint::load_from_kdl(&doc, source)?;
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
                            .find(|pcomp| pcomp.name() == comp.name())
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

/// How to handle this blueprint if there's another node with the same name.
///
/// When merging blueprints you can only change the old blueprint's components;
/// its inheritor, etc are unchangeable once the blueprint is inserted.
#[derive(Debug, Clone, Copy, Default)]
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

#[derive(Debug, Error)]
pub enum BlueprintParseError {
    #[error("error when parsing kdl: {0}")]
    Parse(#[from] kdl::KdlError),
    #[error("error when turning kdl into blueprints: {0}")]
    Deser(#[from] RawBlueprintDeserError),
}

macro_rules! passthru {
    ($($func:ident -> $ret:ty);*) => {
        $(
            fn $func<'a>(&'a self) -> $ret {
                match self {
                    // this can't be done automatically
                    BlueprintParseError::Parse(x) => x.$func(),
                    BlueprintParseError::Deser(x) => x.$func(),
                }
            }
        )*
    };
}

impl Diagnostic for BlueprintParseError {
    passthru! {
        code -> Option<Box<dyn Display + 'a>>;
        severity  -> Option<Severity>;
        help -> Option<Box<dyn Display + 'a>>;
        url -> Option<Box<dyn Display + 'a>>;
        source_code -> Option<&dyn SourceCode>;
        labels -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>>;
        related -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>>;
        diagnostic_source -> Option<&dyn Diagnostic>
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("{kind}")]
pub struct RawBlueprintDeserError {
    #[label]
    pub span: SourceSpan,
    #[source_code]
    pub src: NamedSource,
    pub kind: RawBlueprintParseErrorKind,
}

const BP_REQS: &str = r#"only `merge="merge"` or `merge="clobber"`, and `inherit="some other blueprint"`, are allowed"#;

#[derive(Debug, Error)]
pub enum RawBlueprintParseErrorKind {
    #[error("blueprint node had no children")]
    NoChildren,
    #[error("blueprint node had an argument; {}", BP_REQS)]
    TopLevelArgument,
    #[error("blueprint node had an annotation; {}", BP_REQS)]
    TopLevelAnnotation,
    #[error("blueprint node had an invalid key; {}", BP_REQS)]
    InvalidKey,
    #[error("the `inherit` key didn't have a string value")]
    NonStringInherit,
    #[error(r#"the `merge` key didn't equal "clobber" or "merge""#)]
    BadMerge,
    #[error("redefined `inherit`")]
    ClobberInherit,
    #[error("redefined `merge`")]
    ClobberMerge,
}
