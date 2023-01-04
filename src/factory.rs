use std::marker::PhantomData;

use kdl::KdlNode;
use palkia::prelude::*;
use serde::de::DeserializeOwned;

/// Things that can be deserialized out of a kdl node.
pub trait ComponentFactory<Ctx>:
    ComponentAssembler<Ctx> + Send + Sync + 'static
{
    type Output: Component;

    /// Attempt to load a component out of a node.
    fn load(&self, node: &KdlNode, ctx: &Ctx) -> eyre::Result<Self::Output>;
}

/// Blanket convenience impl
impl<Ctx, T: ComponentFactory<Ctx>> ComponentAssembler<Ctx> for T {
    fn assemble<'a, 'w>(
        &self,
        mut builder: EntityBuilder<'a, 'w>,
        node: &KdlNode,
        ctx: &Ctx,
    ) -> eyre::Result<EntityBuilder<'a, 'w>> {
        let output = self.load(node, ctx)?;
        builder.insert(output);
        Ok(builder)
    }
}

/// A freer way to modify entities based on nodes.
///
/// Each assembler is a singleton object stored in an [`EntityFabricator`].
/// You can use the `&self` param for configuration data, I suppose.
pub trait ComponentAssembler<Ctx>: Send + Sync + 'static {
    /// Attempt to load a component out of a node with full access to the builder.
    /// Go wild.
    fn assemble<'a, 'w>(
        &self,
        builder: EntityBuilder<'a, 'w>,
        node: &KdlNode,
        ctx: &Ctx,
    ) -> eyre::Result<EntityBuilder<'a, 'w>>;
}

/// Convenience wrapper for the common case where you want to just deserialize something from
/// a node with serde.
///
/// Doesn't use the `Ctx` generic (just has it in PhantomData).
pub struct SerdeComponentFactory<T, Ctx>(pub PhantomData<(T, Ctx)>);

impl<T, Ctx> ComponentFactory<Ctx> for SerdeComponentFactory<T, Ctx>
where
    T: DeserializeOwned + Component,
    Ctx: Send + Sync + 'static,
{
    type Output = T;

    fn load(&self, node: &KdlNode, _ctx: &Ctx) -> eyre::Result<Self::Output> {
        let deserd: T = knurdy::deserialize_node(node)?;
        Ok(deserd)
    }
}
