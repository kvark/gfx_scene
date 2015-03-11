extern crate "gfx_phase" as phase;
extern crate gfx;
extern crate cgmath;

#[derive(Debug)]
pub enum Error {
    Batch(gfx::batch::Error),
    Flush(phase::FlushError),
}

/// Abstract scene that can be drawn into something
pub trait AbstractScene<D: gfx::Device> {
    type SpaceData;
    type Entity;
    type Camera;

    fn draw<H: phase::AbstractPhase<D, Self::Entity, Self::SpaceData> + ?Sized>(
            &mut self, &mut H, &Self::Camera, &gfx::Frame<D::Resources>,
            &mut gfx::Renderer<D::Resources, D::CommandBuffer>) -> Result<(), Error>;
}

/// A class that manages spatial relations between objects
pub trait World {
    type Scalar: cgmath::BaseFloat + 'static;
    type Rotation: cgmath::Rotation3<Self::Scalar>;
    type Transform: cgmath::CompositeTransform3<Self::Scalar, Self::Rotation>;
    type NodePtr;
    type SkeletonPtr;
    type Iter: Iterator<Item = Self::Transform>;

    fn get_transform(&self, &Self::NodePtr) -> &Self::Transform;
    fn iter_bones(&self, &Self::SkeletonPtr) -> Self::Iter;
}

pub struct Entity<R: gfx::Resources, M, W: World> {
    pub name: String,
    pub material: M,
    mesh: gfx::Mesh<R>,
    slice: gfx::Slice<R>,
    node: W::NodePtr,
    skeleton: Option<W::SkeletonPtr>,
}

impl<R: gfx::Resources, M, W: World> phase::Entity<R, M> for Entity<R, M, W> {
    fn get_material(&self) -> &M {
        &self.material
    }
    fn get_mesh(&self) -> (&gfx::Mesh<R>, &gfx::Slice<R>) {
        (&self.mesh, &self.slice)
    }
}

pub struct Camera<P, N> {
    pub name: String,
    pub projection: P,
    pub node: N,
}

pub struct Scene<R: gfx::Resources, M, W: World, P> {
    pub entities: Vec<Entity<R, M, W>>,
    pub cameras: Vec<Camera<P, W::NodePtr>>,
    pub world: W,
    context: gfx::batch::Context<R>,
}

pub struct SpaceData<S> {
    pub vertex_mx: cgmath::Matrix4<S>,
    pub normal_mx: cgmath::Matrix3<S>,
}

impl<S: cgmath::BaseFloat> phase::ToDepth for SpaceData<S> {
    type Depth = S;
    fn to_depth(&self) -> S {
        self.vertex_mx.w.z / self.vertex_mx.w.w
    }
}

impl<
    D: gfx::Device,
    M: phase::Material,
    W: World,
    P: cgmath::Projection<W::Scalar>,
> AbstractScene<D> for Scene<D::Resources, M, W, P> {
    type SpaceData = SpaceData<W::Scalar>;
    type Entity = Entity<D::Resources, M, W>;
    type Camera = Camera<P, W::NodePtr>;

    fn draw<H: phase::AbstractPhase<D, Entity<D::Resources, M, W>, SpaceData<W::Scalar>> + ?Sized>(
            &mut self, phase: &mut H, camera: &Camera<P, W::NodePtr>,
            frame: &gfx::Frame<D::Resources>,
            renderer: &mut gfx::Renderer<D::Resources, D::CommandBuffer>)
            -> Result<(), Error> {
        use cgmath::{Matrix, ToMatrix3, ToMatrix4, Transform, ToComponents};
        let cam_inverse = self.world.get_transform(&camera.node)
                                    .invert().unwrap();
        let projection = camera.projection.to_matrix4()
                               .mul_m(&cam_inverse.to_matrix4());
        for entity in self.entities.iter_mut() {
            if !phase.test(entity) {
                 continue
            }
            let model = self.world.get_transform(&entity.node);
            let view = cam_inverse.concat(&model);
            let mvp = projection.mul_m(&model.to_matrix4());
            let (_, rot, _) = view.decompose();
            //TODO: cull `ent.bounds` here
            let data = SpaceData {
                vertex_mx: mvp,
                normal_mx: rot.to_matrix3(),
            };
            match phase.enqueue(entity, data, &mut self.context) {
                Ok(()) => (),
                Err(e) => return Err(Error::Batch(e)),
            }
        }
        phase.flush(frame, &self.context, renderer)
             .map_err(|e| Error::Flush(e))
    }
}

/// Wrapper around a scene that carries a list of phases as well as the
/// `Renderer`, allowing to isolate a command buffer completely.
pub struct PhaseHarness<D: gfx::Device, C: AbstractScene<D>> {
    pub scene: C,
    pub phases: Vec<Box<phase::AbstractPhase<D, C::Entity, C::SpaceData>>>,
    pub renderer: gfx::Renderer<D::Resources, D::CommandBuffer>,
}

impl<
    D: gfx::Device,
    C: AbstractScene<D>,
> PhaseHarness<D, C> {
    pub fn draw(&mut self, camera: &C::Camera, frame: &gfx::Frame<D::Resources>)
                -> Result<(), Error> {
        use std::ops::DerefMut;
        self.renderer.reset();
        for phase in self.phases.iter_mut() {
            match self.scene.draw(phase.deref_mut(), camera, frame, &mut self.renderer) {
                Ok(_) => (),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

pub type PerspectiveCam<W: World> = Camera<
    cgmath::PerspectiveFov<W::Scalar, cgmath::Rad<W::Scalar>>,
    W::NodePtr
>;

/// A typical scene to be used in demoes. Has a heterogeneous vector of phases
/// and a perspective fov-based camera.
pub type StandardScene<
    D: gfx::Device,
    M: phase::Material,
    W: World,
    P: cgmath::Projection<W::Scalar>,
> = PhaseHarness<D, Scene<D::Resources, M, W, P>>;
