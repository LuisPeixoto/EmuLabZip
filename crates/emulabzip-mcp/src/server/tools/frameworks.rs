//! The frameworks tools.

use crate::server::*;

#[tool_router(router = frameworks_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "Which UI frameworks an app is built with, found from its files: Flutter, React Native, Hermes, Jetpack Compose or SwiftUI."
    )]
    async fn detect_frameworks(&self, Parameters(a): Parameters<args::App>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                json(&self.lab.frameworks(&target, &a.app).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "The widget tree of a running Flutter debug or profile app, with the file and line each widget of the app's own code was made at."
    )]
    async fn flutter_widget_tree(&self, Parameters(a): Parameters<args::Flutter>) -> CallToolResult {
        reply(
            async {
                let flutter = self.flutter(&a).await?;
                let root = flutter.widget_tree().await.map_err(|e| e.to_string())?;
                json(&serde_json::json!({ "creationTracked": flutter.creation_tracked(), "tree": root }))
            }
            .await,
        )
    }

    #[tool(
        description = "The WebViews and browsers on an Android device that can be inspected (a WebView in a debuggable app that turned on WebView.setWebContentsDebuggingEnabled, or Chrome), each with its pages: id, title and address."
    )]
    async fn webview_list(&self, Parameters(a): Parameters<args::Device>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                let sockets = self.lab.devtools_sockets(&target).await.map_err(|e| e.to_string())?;
                let mut out = Vec::new();
                for s in sockets {
                    let pages =
                        emulabzip_frameworks::webview::pages(&self.lab, &target, &s.socket).await.unwrap_or_default();
                    out.push(serde_json::json!({ "socket": s, "pages": pages }));
                }
                json(&out)
            }
            .await,
        )
    }

    #[tool(
        description = "What a web page in a WebView or Chrome writes to its console, and the errors it does not catch, over a few seconds (at most 60). Use the page meanwhile, or reload it."
    )]
    async fn webview_console(&self, Parameters(a): Parameters<args::Webview>) -> CallToolResult {
        reply(
            async {
                let seconds = a.seconds.unwrap_or(8);
                if seconds == 0 || seconds > 60 {
                    return Err("seconds is between 1 and 60".to_string());
                }
                for (name, value) in [("app", &a.app), ("socket", &a.socket)] {
                    if let Some(v) = value
                        && v.len() > 120
                    {
                        return Err(format!("{name} is too long"));
                    }
                }
                if let Some(app) = &a.app
                    && !emulabzip_core::control::is_app_id(app)
                {
                    return Err(format!("`{app}` is not a package name"));
                }
                if let Some(socket) = &a.socket
                    && !emulabzip_core::backend::is_socket_name(socket)
                {
                    return Err(format!("`{socket}` is not a socket name"));
                }
                let target = self.target(&a.device).await?;
                let (_, page, view) = emulabzip_frameworks::webview::connect(
                    &self.lab,
                    &target,
                    a.app.as_deref(),
                    a.socket.as_deref(),
                    a.page.as_deref(),
                )
                .await
                .map_err(|e| e.to_string())?;
                let lines = view.console(seconds, 300).await.map_err(|e| e.to_string())?;
                json(&serde_json::json!({ "page": page, "lines": lines }))
            }
            .await,
        )
    }

    #[tool(
        description = "Build and raster times of the frames a Flutter app draws over a few seconds (at most 30), with the janky ones counted. Nothing is drawn while the app is idle, so use it meanwhile."
    )]
    async fn flutter_frames(&self, Parameters(a): Parameters<args::Flutter>) -> CallToolResult {
        reply(
            async {
                let seconds = a.seconds.unwrap_or(8).clamp(1, 30);
                let flutter = self.flutter(&a).await?;
                let mut stats = flutter.frames(seconds).await.map_err(|e| e.to_string())?;
                // The list of every frame is for a chart; an assistant needs the summary.
                stats.timeline.clear();
                json(&stats)
            }
            .await,
        )
    }

    #[tool(description = "The Dart heap of a running Flutter app and the classes that use the most memory.")]
    async fn flutter_memory(&self, Parameters(a): Parameters<args::Flutter>) -> CallToolResult {
        reply(async { json(&self.flutter(&a).await?.memory(15).await.map_err(|e| e.to_string())?) }.await)
    }

    #[tool(
        description = "The state of a React Native development app: Redux stores, React Query caches, React contexts and stores registered in globalThis.__EMULABZIP_STORES__, read through Metro (must be running). Release builds have none of this."
    )]
    async fn react_state(&self, Parameters(a): Parameters<args::ReactNative>) -> CallToolResult {
        reply(
            async {
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                let port = a.metro_port.unwrap_or(emulabzip_frameworks::react_native::METRO_PORT);
                let rn =
                    emulabzip_frameworks::react_native::connect_app(&a.app, port).await.map_err(|e| e.to_string())?;
                Ok(cap(json(&rn.state().await.map_err(|e| e.to_string())?)?, 200_000))
            }
            .await,
        )
    }

    #[tool(
        description = "The React component tree of a React Native development app, with props, read through Metro (must be running). Metro allows one debugger at a time, so this can close React Native DevTools."
    )]
    async fn react_tree(&self, Parameters(a): Parameters<args::ReactNative>) -> CallToolResult {
        reply(
            async {
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                let port = a.metro_port.unwrap_or(emulabzip_frameworks::react_native::METRO_PORT);
                let rn =
                    emulabzip_frameworks::react_native::connect_app(&a.app, port).await.map_err(|e| e.to_string())?;
                json(&rn.tree().await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }
}
