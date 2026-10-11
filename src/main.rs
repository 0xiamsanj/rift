use gpui::*;
use rift::app::RiftApp;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() {
    // Initialize structured tracing (safe logging with secret masking)
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rift=debug,info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .try_init();

    let app = Application::new();
    app.run(|cx: &mut App| {
        let mut options = WindowOptions::default();
        options.window_bounds = Some(WindowBounds::Windowed(Bounds {
            origin: Point::default(),
            size: size(px(1280.0), px(800.0)),
        }));
        options.titlebar = Some(TitlebarOptions {
            title: Some("Rift - Fast Native SFTP Client".into()),
            appears_transparent: false,
            traffic_light_position: None,
        });

        let _ = cx.open_window(options, |_, cx| {
            cx.new(|cx| RiftApp::new(cx))
        });
    });
}
