use std::{ops::Mul, time::Instant};

use eframe::egui_wgpu::{self, wgpu};
use glam::Quat;

use crate::camera::Camera;

#[derive(Debug, Default)]
pub struct Viewport {
    camera: Camera,
    camera_smoothed: Camera,
}

impl Viewport {
    pub fn new<'a>(cc: &'a eframe::CreationContext<'a>) -> Option<Self> {
        let wgpu_render_state = cc.wgpu_render_state.as_ref()?;
        let format = wgpu_render_state.target_format;

        let device = &wgpu_render_state.device;

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::FRAGMENT,
                range: 0..std::mem::size_of::<Uniforms>() as u32,
            }],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            cache: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: None,
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: None,
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            multisample: wgpu::MultisampleState::default(),
            depth_stencil: None,
            multiview: None,
        });

        // Because the graphics pipeline must have the same lifetime as the egui render pass,
        // instead of storing the pipeline in our `Custom3D` struct, we insert it into the
        // `paint_callback_resources` type map, which is stored alongside the render pass.
        wgpu_render_state
            .renderer
            .write()
            .callback_resources
            .insert(RenderResources {
                pipeline,
                t0: Instant::now(),
            });

        Some(Self::default())
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(
            egui::Vec2::new(ui.available_width(), ui.available_height()),
            egui::Sense::hover(),
        );

        let dt = ui.input(|i| i.stable_dt).min(0.1);

        if response.contains_pointer() {
            self.camera.radius /= ui.input(|i| i.zoom_delta());

            let delta = ui.input(|i| i.raw_scroll_delta);
            self.camera.yaw += 0.01 * delta.x as f32;
            self.camera.pitch += 0.01 * delta.y as f32;
        }

        self.camera_smoothed.lerp_exp(&self.camera, 0.9, dt);

        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            RenderCallback {
                camera: self.camera_smoothed,
            },
        ));
    }
}

#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub struct Uniforms {
    rotation: [f32; 4],
    eye: [f32; 3],
    viewport_aspect_ratio: f32,
    max_iterations: u32,
    min_step_size: f32,
    time: f32,
}

// Callbacks in egui_wgpu have 3 stages:
// * prepare (per callback impl)
// * finish_prepare (once)
// * paint (per callback impl)
//
// The prepare callback is called every frame before paint and is given access to the wgpu
// Device and Queue, which can be used, for instance, to update buffers and uniforms before
// rendering.
// If [`egui_wgpu::Renderer`] has [`egui_wgpu::FinishPrepareCallback`] registered,
// it will be called after all `prepare` callbacks have been called.
// You can use this to update any shared resources that need to be updated once per frame
// after all callbacks have been processed.
//
// On both prepare methods you can use the main `CommandEncoder` that is passed-in,
// return an arbitrary number of user-defined `CommandBuffer`s, or both.
// The main command buffer, as well as all user-defined ones, will be submitted together
// to the GPU in a single call.
//
// The paint callback is called after finish prepare and is given access to egui's main render pass,
// which can be used to issue draw commands.
struct RenderCallback {
    camera: Camera,
}

impl egui_wgpu::CallbackTrait for RenderCallback {
    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let resources: &RenderResources = resources.get().unwrap();
        pass.set_pipeline(&resources.pipeline);
        pass.set_push_constants(
            wgpu::ShaderStages::FRAGMENT,
            0,
            as_byte_slice(&[Uniforms {
                rotation: Quat::from_rotation_y(self.camera.yaw)
                    .mul(Quat::from_rotation_x(self.camera.pitch))
                    .into(),
                eye: [0.0, 0.0, -self.camera.radius],
                viewport_aspect_ratio: info.viewport.aspect_ratio(),
                max_iterations: 64,
                min_step_size: 0.001,
                time: resources.t0.elapsed().as_secs_f32(),
            }]),
        );
        // TODO: Render fullscreen triangle instead of a quad
        pass.draw(0..6, 0..1);
    }
}

struct RenderResources {
    pipeline: wgpu::RenderPipeline,
    t0: Instant,
}

#[allow(unused)]
fn as_byte_slice<T>(slice: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            slice.as_ptr() as *const u8,
            slice.len() * std::mem::size_of::<T>(),
        )
    }
}
