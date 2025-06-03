use std::{io::{BufReader, Cursor}, mem};

use log::warn;
use wgpu::util::DeviceExt;

use crate::resources;

pub trait Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub normal: [f32; 3],
}

impl ModelVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
}

impl Vertex for ModelVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ModelVertex::ATTRIBS
        }
    }
}

pub struct Model {
    pub name: String,
    pub index_buffer: wgpu::Buffer,
    pub vertex_buffer: wgpu::Buffer,
    pub num_indices: u32
}

impl Model {
    pub async fn load(
        file_name: &str,
        device: &wgpu::Device
    ) -> anyhow::Result<Model> {
        let obj_text = resources::load_string(file_name).await?;
        let obj_cursor = Cursor::new(obj_text);
        let mut obj_reader = BufReader::new(obj_cursor);

        let (models, _) = tobj::load_obj_buf_async(
            &mut obj_reader,
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
            |p| async {
                unimplemented!("Materials aren't used")
            },
        ).await?;

        if models.len() > 1 {
            warn!("Found more than one model; only using the first.");
        }
        let model = &models[0];
        
        let vertices = (0..model.mesh.positions.len() / 3)
            .map(|i| {
                if model.mesh.normals.is_empty(){
                    ModelVertex {
                        position: [
                            model.mesh.positions[i * 3],
                            model.mesh.positions[i * 3 + 1],
                            model.mesh.positions[i * 3 + 2],
                        ],
                        color: [0., 0., 0.],
                        normal: [0., 0., 0.],
                    }
                }else{
                    ModelVertex {
                        position: [
                            model.mesh.positions[i * 3],
                            model.mesh.positions[i * 3 + 1],
                            model.mesh.positions[i * 3 + 2],
                        ],
                        color: [0., 0., 0.],
                        normal: [
                            model.mesh.normals[i * 3],
                            model.mesh.normals[i * 3 + 1],
                            model.mesh.normals[i * 3 + 2],
                        ],
                    }
                }
            })
            .collect::<Vec<_>>();

        let vertex_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} Vertex Buffer", file_name)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }
        );
        let index_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} Index Buffer", file_name)),
                contents: bytemuck::cast_slice(&model.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            }
        );
        Ok(Model {
            name: file_name.to_string(),
            index_buffer, vertex_buffer,
            num_indices: model.mesh.indices.len() as u32
        })
    }
}

pub trait DrawModel<'a> {
    fn draw_model(&mut self, model: &'a Model);
}
impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a> where 'b: 'a {
    fn draw_model(&mut self, model: &'b Model) {
        self.set_vertex_buffer(0, model.vertex_buffer.slice(..));
        self.set_index_buffer(model.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.draw_indexed(0..model.num_indices, 0, 0..1);
    }
}