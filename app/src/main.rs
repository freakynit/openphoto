use std::path::PathBuf;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("OpenPhoto")
        .with_inner_size([1350.0, 800.0])
        .with_min_inner_size([960.0, 600.0])
        .with_drag_and_drop(true);
    if cfg!(target_os = "macos") {
        // Hide the system title bar and draw our own, matching the options bar color
        viewport = viewport
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false);
    }

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "OpenPhoto",
        options,
        Box::new(|cc| Ok(Box::new(op_ui::OpenPhotoApp::new(cc, files)))),
    )
}
