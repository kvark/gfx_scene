use std;
use cgmath;
use gfx::attrib::Floater;
use gfx;
use gfx::traits::*;
use gfx_phase;
use gfx_scene;

static SCALE: f32 = 10.0;

gfx_vertex!( Vertex {
    a_Pos@ pos: [Floater<i8>; 2],
});

impl Vertex {
    fn new(x: i8, y: i8) -> Vertex {
        Vertex {
            pos: [Floater(x), Floater(y)],
        }
    }
}

gfx_parameters!( Params {
    u_Offset@ offset: [f32; 2],
    u_Color@ color: [f32; 4],
    u_Scale@ scale: f32,
});

static VERTEX_SRC: &'static [u8] = b"
    #version 150 core
    in vec2 a_Pos;
    uniform vec2 u_Offset;
    uniform float u_Scale;
    void main() {
        vec2 pos = (a_Pos + u_Offset)/u_Scale;
        gl_Position = vec4(pos, 0.0, 1.0);
    }
";

static FRAGMENT_SRC: &'static [u8] = b"
    #version 150 core
    uniform vec4 u_Color;
    out vec4 o_Color;
    void main() {
        o_Color = u_Color;
    }
";

// Defining the technique, material, and entity

struct Technique<R: gfx::Resources> {
    program: gfx::handle::Program<R>,
    state: gfx::DrawState,
}

impl<R: gfx::Resources> Technique<R> {
    pub fn new<F: Factory<R>>(factory: &mut F) -> Technique<R> {
        let program = factory.link_program(VERTEX_SRC, FRAGMENT_SRC).unwrap();
        Technique {
            program: program,
            state: gfx::DrawState::new(),
        }
    }
}

struct Material;
impl gfx_phase::Material for Material {}

#[derive(Clone, Copy)]
struct ViewInfo(cgmath::Vector2<f32>);

impl gfx_phase::ToDepth for ViewInfo {
    type Depth = f32;
    fn to_depth(&self) -> f32 {0.0}
}

impl<R: gfx::Resources> gfx_phase::Technique<R, Material, ViewInfo>
for Technique<R> {
    type Kernel = ();
    type Params = Params<R>;

    fn test(&self, _: &gfx::Mesh<R>, _: &Material) -> Option<()> {
        Some(())
    }

    fn compile<'a>(&'a self, _: (), _: &ViewInfo)
                   -> gfx_phase::TechResult<'a, R, Params<R>> {
        (   &self.program,
            Params {
                offset: [0.0; 2],
                color: [0.4, 0.5, 0.6, 0.0],
                scale: SCALE,
                _r: std::marker::PhantomData,
            },
            None,
            &self.state,
        )
    }

    fn fix_params(&self, _: &Material, space: &ViewInfo, params: &mut Params<R>) {
        use cgmath::FixedArray;
        params.offset = *space.0.as_fixed();
    }
}

//----------------------------------------

type Transform<S> = cgmath::Decomposed<S, cgmath::Vector3<S>, cgmath::Quaternion<S>>;

impl gfx_scene::ViewInfo<f32, Transform<f32>> for ViewInfo {
    fn new(_: cgmath::Matrix4<f32>, _: Transform<f32>, model: Transform<f32>) -> ViewInfo {
        ViewInfo(cgmath::Vector2::new(model.disp.x, model.disp.y))
    }
}

struct World;

impl World {
    pub fn add(&mut self, offset: cgmath::Vector2<f32>) -> Transform<f32> {
        cgmath::Decomposed {
            scale: 1.0,
            rot: cgmath::Quaternion::identity(),
            disp: cgmath::vec3(offset.x, offset.y, 0.0),
        }
    }
}

impl gfx_scene::World for World {
    type Scalar = f32;
    type Transform = Transform<f32>;
    type NodePtr = Transform<f32>;
    type SkeletonPtr = ();

    fn get_transform(&self, node: &Transform<f32>) -> Transform<f32> {
        *node
    }
}

//----------------------------------------

pub struct App<R: gfx::Resources> {
    phase: gfx_phase::Phase<R, Material, ViewInfo, Technique<R>, ()>,
    scene: gfx_scene::Scene<R, Material, World, cgmath::Aabb3<f32>, cgmath::Ortho<f32>, ViewInfo>,
    camera: gfx_scene::Camera<cgmath::Ortho<f32>, <World as gfx_scene::World>::NodePtr>,
}

impl<R: gfx::Resources + 'static> App<R> where
    R::Buffer: 'static,
    R::ArrayBuffer: 'static,
    R::Shader: 'static,
    R::Program: 'static,
    R::FrameBuffer: 'static,
    R::Surface: 'static,
    R::Texture: 'static,
    R::Sampler: 'static,
{
    pub fn new<F: gfx::Factory<R>>(factory: &mut F) -> App<R> {
        let vertex_data = [
            Vertex::new(0, 1),
            Vertex::new(0, 0),
            Vertex::new(1, 1),
            Vertex::new(1, 0),
        ];

        let mesh = factory.create_mesh(&vertex_data);
        let slice = mesh.to_slice(gfx::PrimitiveType::TriangleStrip);

        let mut scene = gfx_scene::Scene::new(World);
        let num = 10usize;
        let entities = (0..num).map(|i| {
            use cgmath::{Aabb3, Point3, vec2};
            let angle = (i as f32) / (num as f32) * std::f32::consts::PI * 2.0;
            let offset = vec2(4.0 * angle.cos(), 4.0 * angle.sin());
            gfx_scene::Entity {
                name: format!("entity-{}", i),
                visible: true,
                mesh: mesh.clone(),
                node: scene.world.add(offset),
                skeleton: None,
                bound: Aabb3::new(Point3::new(0f32, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0)),
                fragments: vec![
                    gfx_scene::Fragment::new(Material, slice.clone()),
                ],
            }
        }).collect::<Vec<_>>();
        scene.entities.extend(entities.into_iter());

        let phase = gfx_phase::Phase::new("Main", Technique::new(factory))
                                     .with_sort(gfx_phase::sort::program);

        let camera = gfx_scene::Camera {
            name: "Cam".to_string(),
            projection: cgmath::Ortho {
                left: -SCALE, right: SCALE,
                bottom: -SCALE, top: SCALE,
                near: -1f32, far: 1f32,
            },
            node: scene.world.add(cgmath::Vector2::new(0.0, 0.0))
        };

        App {
            phase: phase,
            scene: scene,
            camera: camera,
        }
    }

    pub fn render<S: gfx::Stream<R>>(&mut self, stream: &mut S) {
        use gfx_scene::AbstractScene;
        let clear_data = gfx::ClearData {
            color: [0.3, 0.3, 0.3, 1.0],
            depth: 1.0,
            stencil: 0,
        };
        stream.clear(clear_data);
        self.scene.draw(&mut self.phase, &self.camera, stream).unwrap();
    }
}
