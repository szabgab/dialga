pub mod blueprint;

use std::collections::{BTreeMap, BTreeSet};

use blueprint::{BlueprintLibrary, BlueprintLookupError, BlueprintParseError};

use kdl::KdlNode;
use knurdy::DeError;
use palkia::{prelude::*, TypeIdWrapper};
use serde::de::DeserializeOwned;
use smol_str::SmolStr;
use thiserror::Error;

/// The entrypoint to the library; a library of blueprints and the ability to instantiate entities from them.
pub struct EntityFabricator {
    blueprints: BlueprintLibrary,
    /// Map component names to factories for it.
    fabricators: BTreeMap<SmolStr, ComponentFactory>,
    known_comp_types: BTreeSet<TypeIdWrapper>,
}

impl EntityFabricator {
    pub fn new() -> Self {
        Self {
            blueprints: BlueprintLibrary::new(),
            fabricators: BTreeMap::new(),
            known_comp_types: BTreeSet::new(),
        }
    }

    /// Register a component type to be loadable from a blueprint.
    ///
    /// Panics if that component type or name has already been registered.
    pub fn register<C: Component + DeserializeOwned>(&mut self, name: &str) {
        let tid = TypeIdWrapper::of::<C>();
        if !self.known_comp_types.insert(tid) {
            panic!("already registered {:?} in the fabricator", tid.type_name);
        }

        let factory = ComponentFactory::new::<C>();
        if let Some(old) = self.fabricators.insert(SmolStr::from(name), factory) {
            panic!("when registering type {:?} under name {:?} to the fabricator, found another type registered under that name {:?}", tid.type_name, name, old.tid.type_name);
        }
    }

    /// Load the KDL string into the fabricator as a list of blueprints.
    ///
    /// The `filepath` argument is just for error reporting purposes; this doesn't load anything from disc.
    pub fn load_str(&mut self, src: &str, filepath: &str) -> Result<(), BlueprintParseError> {
        self.blueprints.load_str(src, filepath)
    }

    /// Instantiate an entity from a blueprint, adding all the components in that blueprint
    /// to the builder.
    ///
    /// Note that the builder doesn't have to be empty! For example, you might want to add a component for
    /// its position before filling it with other information.
    ///
    /// Returns ownership of the builder whether it succeeds or fails, in case you want to insert as many
    /// components as you can before it fails.
    pub fn instantiate_catching_builder<B: EntityBuilder>(
        &self,
        name: &str,
        mut builder: B,
    ) -> Result<B, (InstantiationError, B)> {
        let print = match self.blueprints.lookup(name) {
            Ok(it) => it,
            Err(err) => return Err((err.into(), builder)),
        };

        for comp in print.components {
            let name = comp.name().value();
            let factory = match self.fabricators.get(name) {
                Some(v) => v,
                None => return Err((InstantiationError::NoBlueprint(name.into()), builder)),
            };
            if let Err(err) = (factory.func)(&mut builder, &comp) {
                return Err((InstantiationError::DeError(name.into(), err), builder));
            }
        }

        Ok(builder)
    }

    /// Instantiate an entity, but don't return the builder if something goes wrong.
    ///
    /// See [`EntityFabricator::instantiate_catching_builder`].
    pub fn instantiate<B: EntityBuilder>(
        &self,
        name: &str,
        builder: B,
    ) -> Result<B, InstantiationError> {
        self.instantiate_catching_builder(name, builder)
            .map_err(|(err, _)| err)
    }
}

/// Things that can go wrong when instantiating an entity.
#[derive(Debug, Error)]
pub enum InstantiationError {
    #[error("while looking up the blueprint: {0}")]
    BlueprintLookupError(#[from] BlueprintLookupError),
    #[error("there was no blueprint registered for a component named {0:?}")]
    NoBlueprint(SmolStr),
    #[error("while deserializing the component {0:?} from kdl: {1}")]
    DeError(SmolStr, DeError),
}

/// gah
trait ObjSafeEntityBuilder {
    fn insert_raw(&mut self, component: Box<dyn Component>) -> Option<Box<dyn Component>>;
}
impl<T: EntityBuilder> ObjSafeEntityBuilder for T {
    fn insert_raw(&mut self, component: Box<dyn Component>) -> Option<Box<dyn Component>> {
        <T as EntityBuilder>::insert_raw(self, component)
    }
}

struct ComponentFactory {
    tid: TypeIdWrapper,
    func: Box<
        dyn Fn(&mut dyn ObjSafeEntityBuilder, &KdlNode) -> Result<(), DeError>
            + Send
            + Sync
            + 'static,
    >,
}

impl ComponentFactory {
    fn new<C: Component + DeserializeOwned>() -> Self {
        let clo = |builder: &mut dyn ObjSafeEntityBuilder, node: &KdlNode| -> Result<(), DeError> {
            let component: C = knurdy::deserialize_node(node)?;
            builder.insert_raw(Box::new(component));
            Ok(())
        };
        ComponentFactory {
            tid: TypeIdWrapper::of::<C>(),
            func: Box::new(clo) as _,
        }
    }
}
