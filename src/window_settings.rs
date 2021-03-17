use egui_wgpu_backend::egui;

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct WindowSettings {
    /// outer position of window in physical pixels
    pos: Option<egui::Pos2>,
    /// Inner size of window in logical pixels
    inner_size_points: Option<egui::Vec2>,
}

impl WindowSettings {
    #[cfg(feature = "persistence")]
    pub fn from_json_file(
        settings_json_path: impl AsRef<std::path::Path>,
    ) -> Option<WindowSettings> {
        crate::persistence::read_json(settings_json_path)
    }

    pub fn from_display(window: &winit::window::Window) -> Self {
        let scale_factor = window.scale_factor();
        let inner_size_points = window.inner_size().to_logical::<f32>(scale_factor);

        Self {
            pos: window
                .outer_position()
                .ok()
                .map(|p| egui::pos2(p.x as f32, p.y as f32)),

            inner_size_points: Some(egui::vec2(
                inner_size_points.width as f32,
                inner_size_points.height as f32,
            )),
        }
    }

    pub fn initialize_size(
        &self,
        window: winit::window::WindowBuilder,
    ) -> winit::window::WindowBuilder {
        if let Some(inner_size_points) = self.inner_size_points {
            window.with_inner_size(winit::dpi::LogicalSize {
                width: inner_size_points.x as f64,
                height: inner_size_points.y as f64,
            })
        } else {
            window
        }

        // Not yet available in winit: https://github.com/rust-windowing/winit/issues/1190
        // if let Some(pos) = self.pos {
        //     *window = window.with_outer_pos(glutin::dpi::PhysicalPosition {
        //         x: pos.x as f64,
        //         y: pos.y as f64,
        //     });
        // }
    }

    pub fn restore_positions(&self, window: &winit::window::Window) {
        // not needed, done by `initialize_size`
        // let size = self.size.unwrap_or_else(|| vec2(1024.0, 800.0));
        // display
        //     .gl_window()
        //     .window()
        //     .set_inner_size(glutin::dpi::PhysicalSize {
        //         width: size.x as f64,
        //         height: size.y as f64,
        //     });

        if let Some(pos) = self.pos {
            window.set_outer_position(winit::dpi::PhysicalPosition::new(
                pos.x as f64,
                pos.y as f64,
            ));
        }
    }
}
