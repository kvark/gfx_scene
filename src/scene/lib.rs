#![deny(missing_docs)]

//! Scene infrastructure to be used with Gfx phases.

extern crate gfx_phase;
extern crate gfx;
extern crate cgmath;

use std::fmt::Debug;
use std::marker::PhantomData;

mod cull;

pub use self::cull::{Culler, Frustum, Context};

/// Scene drawing error.
#[derive(Debug)]
pub enum Error {
    /// Error in creating a batch.
    Batch(gfx::batch::Error),
    /// Error in sending a batch for drawing.
    Flush(gfx_phase::FlushError),
}

/// Type of the call counter.
pub type Count = u32;

/// Rendering success report.
#[derive(Clone, Debug)]
pub struct Report {
    /// Number of calls in invisible entities.
    pub calls_invisible: Count,
    /// Number of calls that got culled out.
    pub calls_culled: Count,
    /// Number of calls that the phase doesn't apply to.
    pub calls_rejected: Count,
    /// Number of calls that failed to link batches.
    pub calls_failed: Count,
    /// Number of calls issued to the GPU.
    pub calls_passed: Count,
    /// Number of primitives rendered.
    pub primitives_rendered: Count,
}

impl Report {
    /// Create an empty `Report`.
    pub fn new() -> Report {
        Report {
            calls_rejected: 0,
            calls_failed: 0,
            calls_culled: 0,
            calls_invisible: 0,
            calls_passed: 0,
            primitives_rendered: 0,
        }
    }

    /// Get total number of draw calls.
    pub fn get_calls_total(&self) -> Count {
        self.calls_invisible + self.calls_culled +
        self.calls_rejected  + self.calls_failed +
        self.calls_passed
    }

    /// Get the rendered/submitted calls ratio.
    pub fn get_calls_ratio(&self) -> f32 {
        self.calls_passed as f32 / self.get_calls_total() as f32
    }
}

/// Abstract scene that can be drawn into something.
pub trait AbstractScene<R: gfx::Resources> {
    /// A type of the view information.
    type ViewInfo;
    /// A type of the material.
    type Material;
    /// A type of the camera.
    type Camera;
    /// the status information from the render results
    /// this can be used to communicate meta from the render
    type Status;

    /// Draw the contents of the scene with a specific phase into a stream.
    fn draw<H, S>(&self, &mut H, &Self::Camera, &mut S)
            -> Result<Self::Status, Error> where
        H: gfx_phase::AbstractPhase<R, Self::Material, Self::ViewInfo>,
        S: gfx::Stream<R>;
}

/// A class that manages spatial relations between objects.
pub trait World {
    /// Type of the scalar used in all associated mathematical constructs.
    type Scalar: cgmath::BaseFloat + 'static;
    /// Type of the transform that every node performs relative to the parent.
    type Transform: cgmath::Transform3<Self::Scalar> + Clone;
    /// Pointer to a node, associated with an entity, camera, or something else.
    type NodePtr;
    /// Pointer to a skeleton, associated with an entity.
    type SkeletonPtr;
    /// Get the transformation of a specific node pointer.
    fn get_transform(&self, &Self::NodePtr) -> Self::Transform;
}

/// A fragment of an entity, contains a single draw call.
#[derive(Clone, Debug)]
pub struct Fragment<R: gfx::Resources, M> {
    /// Fragment material.
    pub material: M,
    /// Mesh slice.
    pub slice: gfx::Slice<R>,
}

impl<R: gfx::Resources, M> Fragment<R, M> {
    /// Create a new fragment.
    pub fn new(mat: M, slice: gfx::Slice<R>) -> Fragment<R, M> {
        Fragment {
            material: mat,
            slice: slice,
        }
    }
}

/// A simple struct representing an object with a given material, mesh, bound,
/// and spatial relation to other stuff in the world.
#[derive(Clone, Debug)]
pub struct Entity<R: gfx::Resources, M, W: World, B> {
    /// Name of the entity.
    pub name: String,
    /// Visibility flag.
    pub visible: bool,
    /// Mesh.
    pub mesh: gfx::Mesh<R>,
    /// Node pointer into the world.
    pub node: W::NodePtr,
    /// Skeleton pointer.
    pub skeleton: Option<W::SkeletonPtr>,
    /// Associated spatial bound of the entity.
    pub bound: B,
    /// Vector of fragments, each of a different material.
    pub fragments: Vec<Fragment<R, M>>,
}

impl<R: gfx::Resources, M, W: World, B> Entity<R, M, W, B> {
    /// Create a minimal new `Entity`.
    pub fn new(mesh: gfx::Mesh<R>, node: W::NodePtr, bound: B) -> Entity<R, M, W, B> {
        Entity {
            name: String::new(),
            visible: true,
            mesh: mesh,
            node: node,
            skeleton: None,
            bound: bound,
            fragments: Vec::new(),
        }
    }
}

/// A simple camera with generic projection and spatial relation.
#[derive(Clone, Debug)]
pub struct Camera<P, N> {
    /// Name of the camera.
    pub name: String,
    /// Generic projection.
    pub projection: P,
    /// Generic spatial node.
    pub node: N,
}

impl<
    S: cgmath::BaseFloat + 'static,
    T: Into<cgmath::Matrix4<S>> + cgmath::Transform3<S> + Clone,
    W: World<Scalar = S, Transform = T>,
    P: Into<cgmath::Matrix4<S>> + Clone,
> Camera<P, W::NodePtr> {
    /// Get the view-projection matrix, given the `World`.
    pub fn get_view_projection(&self, world: &W) -> cgmath::Matrix4<S> {
        use cgmath::{Matrix, Transform};
        let node_inverse = world.get_transform(&self.node).invert().unwrap();
        self.projection.clone().into().mul_m(&node_inverse.into())
    }
}

/// Abstract information about the view. Supposed to containt at least
/// Model-View-Projection transform for the shader.
pub trait ViewInfo<S, T: cgmath::Transform3<S>>: gfx_phase::ToDepth<Depth = S> {
    /// Construct a new information block.
    fn new(mvp: cgmath::Matrix4<S>, view: T, model: T) -> Self;
}

/// An example scene type.
pub struct Scene<R: gfx::Resources, M, W: World, B, P, V> {
    /// A list of entities in the scene.
    pub entities: Vec<Entity<R, M, W, B>>,
    /// A list of cameras. It's not really useful, but `P` needs to be
    /// constrained in order to be able to implement `AbstractScene`.
    pub cameras: Vec<Camera<P, W::NodePtr>>,
    /// Spatial world.
    pub world: W,
    _view_dummy: PhantomData<V>,
}

impl<R: gfx::Resources, M, W: World, B, P, V> Scene<R, M, W, B, P, V> {
    /// Create a new empty scene.
    pub fn new(world: W) -> Scene<R, M, W, B, P, V> {
        Scene {
            entities: Vec::new(),
            cameras: Vec::new(),
            world: world,
            _view_dummy: PhantomData,
        }
    }
}

impl<
    R: gfx::Resources,
    M: gfx_phase::Material,
    W: World,
    B: cgmath::Bound<W::Scalar> + Debug,
    P: cgmath::Projection<W::Scalar> + Clone,
    V: ViewInfo<W::Scalar, W::Transform>,
> AbstractScene<R> for Scene<R, M, W, B, P, V> {
    type ViewInfo = V;
    type Material = M;
    type Camera = Camera<P, W::NodePtr>;
    type Status = Report;

    fn draw<H, S>(&self, phase: &mut H, camera: &Camera<P, W::NodePtr>,
            stream: &mut S) -> Result<Report, Error> where
        H: gfx_phase::AbstractPhase<R, M, V>,
        S: gfx::Stream<R>,
    {
        let mut culler = Frustum::new();
        Context::new(&self.world, &mut culler, camera)
                .draw(self.entities.iter(), phase, stream)
    }
}

/// A simple perspective camera based on the `World` trait.
pub type PerspectiveCam<W: World> = Camera<
    cgmath::PerspectiveFov<W::Scalar, cgmath::Rad<W::Scalar>>,
    W::NodePtr
>;
