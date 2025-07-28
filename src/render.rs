use std::sync::Arc;
use std::time::Instant;

use glam::{Mat4, Quat, Vec3, Vec3A, Vec4};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};
use winit::window::Window;

use crate::camera::Camera;

#[derive(Debug)]
pub struct Renderer {
    surface: Surface<'static>,
    config: SurfaceConfiguration,
    device: Device,
    queue: Queue,
    depth_texture: Texture,
    sdf_renderer: SdfRenderer,
    mesh_renderer: MeshRenderer,
    depth_renderer: DepthRenderer,
}

#[derive(Debug)]
pub struct SdfRenderer {
    pipeline: RenderPipeline,
    uniform_buffer: Buffer,
    t0: Instant,
}

#[derive(Debug)]
pub struct MeshRenderer {
    pipeline: RenderPipeline,
    uniform_buffer: Buffer,
    vertex_position_buffer: Buffer,
    vertex_color_buffer: Buffer,
}

#[derive(Debug)]
pub struct DepthRenderer {
    pipeline: RenderPipeline,
    sampler: Sampler,
}

#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub struct SdfUniforms {
    rotation: Quat,
    eye: Vec3,
    viewport_aspect_ratio: f32,
    time: f32,
}

#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub struct MeshUniforms {
    model: Mat4,
    view: Mat4,
    projection: Mat4,
}

fn as_byte_slice<T>(slice: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            slice.as_ptr() as *const u8,
            slice.len() * std::mem::size_of::<T>(),
        )
    }
}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = Instance::new(&InstanceDescriptor::default());
        let surface = instance
            .create_surface(window.clone())
            .expect("Cannot create surface");
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("No GPU available");

        println!("GPU: {}", adapter.get_info().name);
        println!("Render Backend: {:?}", adapter.get_info().backend);

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor::default())
            .await
            .unwrap();

        let config = surface
            .get_default_config(
                &adapter,
                window.inner_size().width,
                window.inner_size().height,
            )
            .expect("Adapter does not support creation of surface");

        println!("Surface format: {:?}", config.format);

        surface.configure(&device, &config);

        let depth_texture = device.create_texture(
            &(TextureDescriptor {
                label: None,
                size: Extent3d {
                    width: config.width,
                    height: config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Depth24Plus,
                view_formats: &[],
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            }),
        );

        let sdf_renderer = SdfRenderer::new(&device, &config);
        let mesh_renderer = MeshRenderer::new(&device, &config);
        let depth_renderer = DepthRenderer::new(&device, &config);

        Renderer {
            surface,
            config,
            device,
            queue,
            depth_texture,
            sdf_renderer,
            mesh_renderer,
            depth_renderer,
        }
    }

    pub fn render(&mut self, camera: &Camera) {
        let surface_texture = self
            .surface
            .get_current_texture()
            .expect("Cannot get next texture");
        let surface_texture_view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());
        let depth_texture_view = self.depth_texture.create_view(&TextureViewDescriptor {
            label: Some("Depth Texture View"),
            ..Default::default()
        });

        let mut encoder = self.device.create_command_encoder(&Default::default());

        self.sdf_renderer.render(
            camera,
            &self.config,
            &surface_texture_view,
            &depth_texture_view,
            &self.device,
            &self.queue,
            &mut encoder,
        );

        self.mesh_renderer.render(
            camera,
            &self.config,
            &surface_texture_view,
            &depth_texture_view,
            &self.device,
            &self.queue,
            &mut encoder,
        );

        // self.depth_renderer.render(
        //     &self.device,
        //     &surface_texture_view,
        //     &depth_texture_view,
        //     &mut encoder,
        // );

        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);

        self.depth_texture = self.device.create_texture(
            &(wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TextureFormat::Depth24Plus,
                view_formats: &[],
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            }),
        );
    }
}

impl SdfRenderer {
    pub fn new(device: &Device, config: &SurfaceConfiguration) -> Self {
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            size: std::mem::size_of::<SdfUniforms>().max(48) as u64,
            mapped_at_creation: false,
        });

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(include_str!("shader/sdf.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            cache: None,
            layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor {
                bind_group_layouts: &[&device.create_bind_group_layout(
                    &BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[BindGroupLayoutEntry {
                            binding: 0,
                            visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                            ty: BindingType::Buffer {
                                ty: BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        }],
                    },
                )],
                ..Default::default()
            })),
            vertex: VertexState {
                module: &shader_module,
                entry_point: None,
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: None,
                targets: &[Some(ColorTargetState {
                    format: config.format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multiview: None,
        });

        SdfRenderer {
            pipeline,
            uniform_buffer,
            t0: Instant::now(),
        }
    }

    pub fn render(
        &mut self,
        camera: &Camera,
        config: &SurfaceConfiguration,
        surface_texture: &TextureView,
        depth_texture: &TextureView,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
    ) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            as_byte_slice(&[SdfUniforms {
                rotation: Quat::from_rotation_y(camera.yaw) * Quat::from_rotation_x(camera.pitch),
                eye: Vec3::new(0.0, 0.0, -camera.radius),
                viewport_aspect_ratio: config.width as f32 / config.height as f32,
                time: self.t0.elapsed().as_secs_f32(),
                // projection: {
                //     let fovy = 60.0_f32.to_radians();
                //     let near = 0.1;
                //     let far = 100.0;

                //     let aspect = self.config.width as f32 / self.config.height as f32;
                //     let tan_half_fovy = (0.5 * fovy).tan();
                //     Mat4::from_cols(
                //         Vec4::new(1.0 / (aspect * tan_half_fovy), 0.0, 0.0, 0.0),
                //         Vec4::new(0.0, 1.0 / tan_half_fovy, 0.0, 0.0),
                //         Vec4::new(0.0, 0.0, -(far + near) / (far - near), -1.0),
                //         Vec4::new(0.0, 0.0, -2.0 * far * near / (far - near), 0.0),
                //     )
                // },
            }]),
        );

        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &surface_texture,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color {
                        r: 0.01,
                        g: 0.01,
                        b: 0.01,
                        a: 1.0,
                    }),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &depth_texture,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(
            0,
            &device.create_bind_group(&BindGroupDescriptor {
                label: None,
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                }],
            }),
            &[],
        );
        pass.set_pipeline(&self.pipeline);
        pass.draw(0..6, 0..1);
    }
}

impl MeshRenderer {
    pub fn new(device: &Device, config: &SurfaceConfiguration) -> Self {
        let positions: [[_; 6]; 6] = core::array::from_fn(|i| {
            let sign_i = i >= 3;

            let i = i % 3;
            let j = (i + 1) % 3;
            let k = (i + 2) % 3;

            fn set_sign_bit(float: &mut f32, sign: bool) {
                unsafe {
                    let float = std::mem::transmute::<_, &mut u32>(float);
                    *float = (*float & !(1 << 31)) | ((!sign as u32) << 31);
                }
            }

            // Each cube vertex coordinate is either positive or negative one
            let mut v = Vec3A::ONE;
            set_sign_bit(&mut v[i], sign_i);

            // Encoded signs of six vertices, three for each triangle
            let mut sign_bits_j = 0b010110;
            let mut sign_bits_k = 0b110100;
            if !sign_i {
                // Winding needs to be inverted
                (sign_bits_k, sign_bits_j) = (sign_bits_j, sign_bits_k);
            }

            core::array::from_fn(|s| {
                let sign_bit_j = (sign_bits_j & (1 << s)) != 0;
                let sign_bit_k = (sign_bits_k & (1 << s)) != 0;
                set_sign_bit(&mut v[j], sign_bit_j);
                set_sign_bit(&mut v[k], sign_bit_k);
                v
            })
        });

        let colors: [_; 6] = core::array::from_fn(|i| {
            let mut v = Vec3A::ZERO;
            for j in 0..3 {
                // Add one so we don't start with black
                if (i + 1) & (1 << j) != 0 {
                    v[j] = 1.0;
                }
            }
            [v; 6]
        });

        let vertex_position_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: as_byte_slice(&positions),
            usage: BufferUsages::VERTEX,
        });

        let vertex_color_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: as_byte_slice(&colors),
            usage: BufferUsages::VERTEX,
        });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            size: std::mem::size_of::<MeshUniforms>() as u64,
            mapped_at_creation: false,
        });

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(include_str!("shader/mesh.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            cache: None,
            layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor {
                bind_group_layouts: &[&device.create_bind_group_layout(
                    &BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[BindGroupLayoutEntry {
                            binding: 0,
                            visibility: ShaderStages::VERTEX,
                            ty: BindingType::Buffer {
                                ty: BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        }],
                    },
                )],
                ..Default::default()
            })),
            vertex: VertexState {
                module: &shader_module,
                entry_point: None,
                buffers: &[
                    VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vec3A>() as BufferAddress,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &[VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: VertexFormat::Float32x4,
                        }],
                    },
                    VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vec3A>() as BufferAddress,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &[VertexAttribute {
                            offset: 0,
                            shader_location: 1,
                            format: VertexFormat::Float32x4,
                        }],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: None,
                targets: &[Some(ColorTargetState {
                    format: config.format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multiview: None,
        });

        MeshRenderer {
            pipeline,
            uniform_buffer,
            vertex_position_buffer,
            vertex_color_buffer,
        }
    }

    pub fn render(
        &mut self,
        camera: &Camera,
        config: &SurfaceConfiguration,
        surface_texture: &TextureView,
        depth_texture: &TextureView,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
    ) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            as_byte_slice(&[MeshUniforms {
                model: Mat4::IDENTITY,
                view: camera.matrix(),
                projection: {
                    let fovy = 45.0_f32.to_radians();
                    let near = 0.1;
                    let far = 100.0;

                    let aspect = config.width as f32 / config.height as f32;
                    let tan_half_fovy = (0.5 * fovy).tan();
                    Mat4::from_cols(
                        Vec4::new(1.0 / (aspect * tan_half_fovy), 0.0, 0.0, 0.0),
                        Vec4::new(0.0, 1.0 / tan_half_fovy, 0.0, 0.0),
                        Vec4::new(0.0, 0.0, -(far + near) / (far - near), -1.0),
                        Vec4::new(0.0, 0.0, -2.0 * far * near / (far - near), 0.0),
                    )
                },
            }]),
        );

        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &surface_texture,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &depth_texture,
                depth_ops: Some(Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(
            0,
            &device.create_bind_group(&BindGroupDescriptor {
                label: None,
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                }],
            }),
            &[],
        );
        pass.set_vertex_buffer(0, self.vertex_position_buffer.slice(..));
        pass.set_vertex_buffer(1, self.vertex_color_buffer.slice(..));
        pass.set_pipeline(&self.pipeline);
        pass.draw(0..36, 0..1);
    }
}

impl DepthRenderer {
    pub fn new(device: &Device, config: &SurfaceConfiguration) -> Self {
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(include_str!("shader/depth.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            cache: None,
            layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor {
                bind_group_layouts: &[&device.create_bind_group_layout(
                    &BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[
                            BindGroupLayoutEntry {
                                binding: 0,
                                visibility: ShaderStages::FRAGMENT,
                                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                                count: None,
                            },
                            BindGroupLayoutEntry {
                                binding: 1,
                                visibility: ShaderStages::FRAGMENT,
                                ty: BindingType::Texture {
                                    sample_type: TextureSampleType::Depth,
                                    view_dimension: TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                        ],
                    },
                )],
                ..Default::default()
            })),
            vertex: VertexState {
                module: &shader_module,
                entry_point: None,
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: None,
                targets: &[Some(ColorTargetState {
                    format: config.format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            depth_stencil: None,
            multiview: None,
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Depth Sampler"),
            ..Default::default()
        });

        DepthRenderer { pipeline, sampler }
    }

    pub fn render(
        &self,
        device: &Device,
        surface_texture: &TextureView,
        depth_texture: &TextureView,
        encoder: &mut CommandEncoder,
    ) {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &surface_texture,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_bind_group(
            0,
            &device.create_bind_group(&BindGroupDescriptor {
                label: None,
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::Sampler(&self.sampler),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(depth_texture),
                    },
                ],
            }),
            &[],
        );
        pass.set_pipeline(&self.pipeline);
        pass.draw(0..3, 0..1);
    }
}
