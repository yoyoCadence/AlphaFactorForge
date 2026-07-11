// Native pop-out windows (Slice 8b). Window creation stays in Rust so labels,
// routes, sizing, and the single-instance/focus behavior have one trusted path.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::error::{AppError, AppResult};

struct PopoutSpec {
    label: &'static str,
    title: &'static str,
    route: &'static str,
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
}

fn popout_spec(kind: &str) -> AppResult<PopoutSpec> {
    match kind {
        "chart" => Ok(PopoutSpec {
            label: "chart-popout-window",
            title: "AlphaFactorForge — Chart",
            route: "index.html?window=chart",
            width: 1100.0,
            height: 720.0,
            min_width: 640.0,
            min_height: 420.0,
        }),
        _ => Err(AppError::Other(format!(
            "unsupported pop-out window kind: {kind}"
        ))),
    }
}

/// Open or focus a single native pop-out window. This command must remain async:
/// Tauri documents a WebView2 deadlock on Windows when WebviewWindowBuilder is
/// called from a synchronous command/event handler.
#[tauri::command]
pub async fn open_popout_window(app: AppHandle, kind: String) -> AppResult<()> {
    let spec = popout_spec(&kind)?;
    if let Some(existing) = app.get_webview_window(spec.label) {
        existing
            .set_focus()
            .map_err(|e| AppError::Other(format!("failed to focus pop-out window: {e}")))?;
        return Ok(());
    }

    WebviewWindowBuilder::new(&app, spec.label, WebviewUrl::App(spec.route.into()))
        .title(spec.title)
        .inner_size(spec.width, spec.height)
        .min_inner_size(spec.min_width, spec.min_height)
        .resizable(true)
        .focused(true)
        .build()
        .map_err(|e| AppError::Other(format!("failed to create pop-out window: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_popout_spec_has_stable_label_and_child_route() {
        let spec = popout_spec("chart").unwrap();
        assert_eq!(spec.label, "chart-popout-window");
        assert_eq!(spec.route, "index.html?window=chart");
        assert!(spec.width >= spec.min_width);
        assert!(spec.height >= spec.min_height);
    }

    #[test]
    fn rejects_unknown_popout_kinds() {
        let err = popout_spec("metrics")
            .err()
            .expect("metrics is deferred to Slice 8b-2");
        assert!(err.to_string().contains("unsupported pop-out window kind"));
    }
}
