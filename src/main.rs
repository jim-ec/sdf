#![allow(unused)]

// TODO: Replace with glam
use cgmath::{Vector3, Zero, vec3};

mod camera;
mod viewport;

fn main() -> eframe::Result {
    eframe::run_native(
        env!("CARGO_PKG_NAME"),
        eframe::NativeOptions {
            persist_window: true,
            viewport: egui::ViewportBuilder::default(),
            wgpu_options: egui_wgpu::WgpuConfiguration {
                wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(egui_wgpu::WgpuSetupCreateNew {
                    power_preference: egui_wgpu::wgpu::PowerPreference::HighPerformance,
                    device_descriptor: std::sync::Arc::new(|adapter| {
                        egui_wgpu::wgpu::DeviceDescriptor {
                            required_features: egui_wgpu::wgpu::Features::PUSH_CONSTANTS,
                            required_limits: egui_wgpu::wgpu::Limits {
                                max_push_constant_size: adapter.limits().max_push_constant_size,
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },

            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(MyApp::new(cc)))),
    )
}

struct MyApp {
    viewport: viewport::Viewport,
}

impl MyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        println!(
            "GPU: {}",
            cc.wgpu_render_state
                .as_ref()
                .unwrap()
                .adapter
                .get_info()
                .name
        );

        Self {
            viewport: viewport::Viewport::new(cc).unwrap(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(ctx.style().visuals.panel_fill))
            .show(&ctx, |ui| {
                self.viewport.ui(ui);
            });
    }
}
