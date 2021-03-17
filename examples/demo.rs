fn main() {
    let app = Box::new(egui_demo_lib::WrapApp::default());

    egui_winit_wgpu_integrator::run(app);
}
