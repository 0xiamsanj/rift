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
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::default(),
                size: size(px(1280.0), px(800.0)),
            })),
            titlebar: Some(TitlebarOptions {
                title: Some("Rift - Fast Native SFTP Client".into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            ..Default::default()
        };

        let _ = cx.open_window(options, |_, cx| {
            let view = cx.new(RiftApp::new);
            let weak = view.downgrade();
            view.update(cx, |app, _| app.set_self_entity(weak));
            view
        });
    });
}
