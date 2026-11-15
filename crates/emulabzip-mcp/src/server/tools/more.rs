//! More tools: readable crashes, React Native, making devices, deep links, flows, and the quality checks that act on a device.

use emulabzip_core::automation::{self, Flow};
use emulabzip_core::links;
use emulabzip_symbols::store::Store;
use emulabzip_symbols::symbolicate::{Source, symbolicate_text};

use crate::server::*;

const MAX_STACK: usize = 100_000;
const MAX_SETTINGS: usize = 8;

fn platform_of(text: &str) -> Result<emulabzip_core::Platform, String> {
    match text {
        "android" => Ok(emulabzip_core::Platform::Android),
        "ios" => Ok(emulabzip_core::Platform::Ios),
        other => Err(format!("`{other}` is not a platform; use android or ios")),
    }
}

#[tool_router(router = more_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    // ---- crashes ---------------------------------------------------------------------------

    #[tool(
        description = "Make a pasted stack readable: Java frames are read with the app's `mapping.txt` (R8), JavaScript frames with its source map, for exactly the version given. `list_mappings` shows what is stored. Crashes from get_crash are read this way already when a mapping is stored.",
        title = "Read a stack with its mapping",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn symbolicate_stack(&self, Parameters(a): Parameters<args::SymbolicateStack>) -> CallToolResult {
        reply(
            async {
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                if a.stack.len() > MAX_STACK {
                    return Err(format!("the stack is longer than {MAX_STACK} characters"));
                }
                let found = Store::default().find(&a.app, Some(&a.version));
                if found.is_empty() {
                    return Err(format!("nothing is stored for {} {}; list_mappings shows what is", a.app, a.version));
                }
                let sources: Vec<Source> = found.iter().map(Source::from).collect();
                let text = symbolicate_text(&a.stack, &sources)?;
                json(&serde_json::json!({ "readable": text, "used": found.iter().map(|s| s.kind.label()).collect::<Vec<_>>() }))
            }
            .await,
        )
    }

    #[tool(
        description = "The mappings, source maps and dSYMs EmuLabZip has stored to read crashes with, by app and build version.",
        title = "List stored mappings",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_mappings(&self, Parameters(a): Parameters<args::ListMappings>) -> CallToolResult {
        reply(json(&Store::default().list(a.app.as_deref())))
    }

    #[tool(
        description = "Store a mapping.txt, a source map or a .dSYM (by its full path on this computer) for an app and build version, so crashes of that build read clearly from now on. The file is copied.",
        title = "Store a mapping",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn add_mapping(&self, Parameters(a): Parameters<args::AddMapping>) -> CallToolResult {
        reply(
            async {
                let path = std::path::PathBuf::from(&a.file);
                if !path.is_absolute() || !path.exists() {
                    return Err(format!("there is nothing at {}; give its full path", a.file));
                }
                let stored = Store::default().add(&a.app, &a.version, &path, None).map_err(|e| e.to_string())?;
                json(&stored)
            }
            .await,
        )
    }

    // ---- React Native ----------------------------------------------------------------------

    #[tool(
        description = "A React Native app's JavaScript console for a few seconds (at most 60): logs, warnings and errors. Needs a development build running with Metro. Use watch_start react_console to keep following it.",
        title = "React Native console",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn react_console(&self, Parameters(a): Parameters<args::ReactConsole>) -> CallToolResult {
        reply(
            async {
                let seconds = a.seconds.unwrap_or(8);
                if seconds == 0 || seconds > 60 {
                    return Err("seconds is between 1 and 60".to_string());
                }
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                let port = a.metro_port.unwrap_or(emulabzip_frameworks::react_native::METRO_PORT);
                let rn =
                    emulabzip_frameworks::react_native::connect_app(&a.app, port).await.map_err(|e| e.to_string())?;
                json(&rn.console(seconds).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "How much memory a React Native app's Hermes engine uses, and which version. Needs a development build running with Metro.",
        title = "React Native Hermes heap",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn react_heap(&self, Parameters(a): Parameters<args::ReactApp>) -> CallToolResult {
        reply(
            async {
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                let port = a.metro_port.unwrap_or(emulabzip_frameworks::react_native::METRO_PORT);
                let rn =
                    emulabzip_frameworks::react_native::connect_app(&a.app, port).await.map_err(|e| e.to_string())?;
                json(&rn.heap().await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    // ---- devices ---------------------------------------------------------------------------

    #[tool(
        description = "What a new emulator or simulator can be made from: the system images or runtimes, and the device models, by id, with whether the image is installed.",
        title = "Choices for a new device",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn device_options(&self, Parameters(a): Parameters<args::DeviceOptionsArgs>) -> CallToolResult {
        reply(
            async {
                let platform = platform_of(&a.platform)?;
                json(&self.lab.device_options(platform).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "Make a new emulator or simulator from an image and a model that `device_options` lists. It is not started: boot_device starts it. Nothing existing is changed.",
        title = "Create a device",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn create_device(&self, Parameters(a): Parameters<args::CreateDeviceArgs>) -> CallToolResult {
        reply(
            async {
                let platform = platform_of(&a.platform)?;
                let request = emulabzip_core::lifecycle::CreateDevice { name: a.name, image: a.image, model: a.model };
                if !request.is_valid() {
                    return Err("the name, image or model is not valid: use the ids device_options shows".into());
                }
                json(&self.lab.create_device(platform, &request).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "What differs between two devices: build, hardware, display, locale and the rest of the facts device_info reports, side by side. Good for \"why does it only break on this one\".",
        title = "Compare two devices",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn compare_devices(&self, Parameters(a): Parameters<args::CompareDevices>) -> CallToolResult {
        reply(
            async {
                let (da, db) = (self.lab.resolve(Some(&a.a)).await.map_err(|e| e.to_string())?, self.lab.resolve(Some(&a.b)).await.map_err(|e| e.to_string())?);
                let (ta, tb) = (da.target(), db.target());
                let (ia, ib) = tokio::join!(self.lab.info(&ta), self.lab.info(&tb));
                let rows = emulabzip_core::diff(&ia.map_err(|e| e.to_string())?, &ib.map_err(|e| e.to_string())?);
                let all = a.all.unwrap_or(false);
                let shown: Vec<_> = rows.iter().filter(|r| all || !r.same).collect();
                json(&serde_json::json!({ "a": da.name, "b": db.name, "differing": rows.iter().filter(|r| !r.same).count(), "total": rows.len(), "rows": shown }))
            }
            .await,
        )
    }

    // ---- links -----------------------------------------------------------------------------

    #[tool(
        description = "The deep links opened and checked before, newest first, with the result of the last check and whether it is pinned.",
        title = "Deep link history",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn link_history(&self) -> CallToolResult {
        reply(json(&links::load_history()))
    }

    #[tool(
        description = "Ask Android to verify an app's domains again (`pm verify-app-links --re-verify`), then run check_link after a few seconds to see whether it worked.",
        title = "Verify an app's links again",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = true)
    )]
    async fn verify_app_links(&self, Parameters(a): Parameters<args::VerifyLinks>) -> CallToolResult {
        reply(
            self.control(&a.device, Control::VerifyAppLinks { app: a.app }, "Android was asked to verify again").await,
        )
    }

    // ---- flows -----------------------------------------------------------------------------

    #[tool(
        description = "The steps of a saved flow, with their labels, and the screen size it was recorded on.",
        title = "Read a flow",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn flow_show(&self, Parameters(a): Parameters<args::FlowId>) -> CallToolResult {
        reply(automation::load(&a.flow).map_err(|e| e.to_string()).and_then(|f| json(&f)))
    }

    #[tool(
        description = "A saved flow as a Maestro YAML flow (`maestro`) or as a shell script of emulabzip commands (`script`), returned as text.",
        title = "Export a flow",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn flow_export(&self, Parameters(a): Parameters<args::FlowExport>) -> CallToolResult {
        reply(automation::load(&a.flow).map_err(|e| e.to_string()).and_then(|f| match a.format.as_str() {
            "maestro" => Ok(f.to_maestro()),
            "script" => Ok(f.to_script()),
            other => Err(format!("`{other}` is not a format: use maestro or script")),
        }))
    }

    #[tool(
        description = "Write a flow by hand: a list of steps, each an object with a `type` (launch; tap {x,y}; longPress {x,y,ms}; swipe {x1,y1,x2,y2,ms}; text {text}; key {key}; wait {ms}) that play_flow can run, and flow_export can turn into Maestro. Taps are in pixels. A flow with that name must not exist yet.",
        title = "Create a flow",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn flow_create(&self, Parameters(a): Parameters<args::FlowCreate>) -> CallToolResult {
        reply(
            async {
                if a.steps.len() > 500 {
                    return Err("at most 500 steps".to_string());
                }
                let mut flow = Flow::new(&a.name, a.app.as_deref(), a.width.zip(a.height));
                for (i, step) in a.steps.into_iter().enumerate() {
                    flow.steps.push(serde_json::from_value(step).map_err(|e| format!("step {} is not valid: {e}", i + 1))?);
                }
                flow.validate().map_err(|e| e.to_string())?;
                let path = automation::flows_dir().join(format!("{}.json", automation::slug(&a.name)));
                if path.exists() {
                    return Err(format!("a flow called {} exists; choose another name", automation::slug(&a.name)));
                }
                let saved = automation::save(&flow).map_err(|e| e.to_string())?;
                json(&serde_json::json!({ "flow": saved.file_stem().map(|s| s.to_string_lossy()), "steps": flow.steps.len() }))
            }
            .await,
        )
    }

    // ---- WebViews --------------------------------------------------------------------------

    #[tool(
        description = "Reload a WebView page, ignoring its cache.",
        title = "Reload a web page",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn webview_reload(&self, Parameters(a): Parameters<args::WebviewReload>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                let (_, _, view) = emulabzip_frameworks::webview::connect(
                    &self.lab,
                    &target,
                    a.app.as_deref(),
                    a.socket.as_deref(),
                    a.page.as_deref(),
                )
                .await
                .map_err(|e| e.to_string())?;
                view.reload().await.map_err(|e| e.to_string())?;
                Ok("Reloaded".to_string())
            }
            .await,
        )
    }

    // ---- quality ---------------------------------------------------------------------------

    #[tool(
        description = "Keep the screen as a baseline for visual_compare: it is saved as a PNG in the project's `visual` folder. A baseline that exists is not replaced.",
        title = "Save a visual baseline",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn visual_capture(&self, Parameters(a): Parameters<args::VisualCapture>) -> CallToolResult {
        reply(
            async {
                if !emulabzip_audit::visual::is_baseline_name(&a.name) {
                    return Err(format!("`{}` is not a baseline name: use letters, digits, . _ and -", a.name));
                }
                let dir = visual_dir()?;
                let shot = self.lab.screenshot(&self.target(&a.device).await?).await.map_err(|e| e.to_string())?;
                let path =
                    emulabzip_audit::visual::save_baseline(&dir, &a.name, &shot, false).map_err(|e| e.to_string())?;
                json(&serde_json::json!({ "baseline": a.name, "path": path }))
            }
            .await,
        )
    }

    #[tool(
        description = "The visual baselines saved in the project's `visual` folder, with their sizes.",
        title = "List visual baselines",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn visual_list(&self) -> CallToolResult {
        reply(visual_dir().and_then(|dir| {
            json(
                &emulabzip_audit::visual::list_baselines(&dir)
                    .into_iter()
                    .map(|(n, s)| serde_json::json!({ "name": n, "bytes": s }))
                    .collect::<Vec<_>>(),
            )
        }))
    }

    #[tool(
        description = "Try the screen in other languages and text sizes and say what gets worse: text that is cut off, off screen, or not mirrored in a right-to-left language, and touch targets that shrink. The language and text size are changed on the device and put back at the end. Android needs `app` to change its language. Takes a while: about ten seconds per setting.",
        title = "Sweep languages and text sizes",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn i18n_sweep(&self, Parameters(a): Parameters<args::I18nSweep>) -> CallToolResult {
        reply(
            async {
                let locales = a.locales.unwrap_or_default();
                let scales = a.text_sizes.unwrap_or_default();
                if locales.len() > MAX_SETTINGS || scales.len() > MAX_SETTINGS {
                    return Err(format!(
                        "try at most {MAX_SETTINGS} languages and {MAX_SETTINGS} text sizes at a time"
                    ));
                }
                if let Some(l) = locales.iter().find(|l| !emulabzip_core::control::is_locale_tag(l)) {
                    return Err(format!("`{l}` is not a language tag such as de-DE, en-XA or ar-XB"));
                }
                if let Some(s) = scales.iter().find(|s| !(50..=300).contains(*s)) {
                    return Err(format!("a text size is between 50 and 300 percent, not {s}"));
                }
                if let Some(app) = &a.app
                    && !emulabzip_core::control::is_app_id(app)
                {
                    return Err(format!("`{app}` is not a package name or bundle id"));
                }
                let target = self.target(&a.device).await?;
                let plan = emulabzip_audit::sweep::Plan {
                    locales,
                    scales,
                    app: a.app,
                    settle: std::time::Duration::from_millis(1500),
                };
                let sweep = emulabzip_audit::sweep::run(
                    &emulabzip_audit::sweep::LabScreen { lab: &self.lab, target: &target },
                    &plan,
                )
                .await
                .map_err(|e| e.to_string())?;
                json(&sweep)
            }
            .await,
        )
    }

    #[tool(
        description = "Where an app's memory grew: reads its memory, waits (while you use the app), reads again, and returns the categories that changed, largest first. A leak shows as a category that only goes up.",
        title = "Memory change over time",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn memory_diff(&self, Parameters(a): Parameters<args::MemoryDiff>) -> CallToolResult {
        reply(
            async {
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                let seconds = a.seconds.unwrap_or(15).clamp(1, 120);
                let target = self.target(&a.device).await?;
                let before = self.lab.memory(&target, &a.app).await.map_err(|e| e.to_string())?;
                tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
                let after = self.lab.memory(&target, &a.app).await.map_err(|e| e.to_string())?;
                json(&serde_json::json!({ "seconds": seconds, "changes": emulabzip_core::memory::diff(&before, &after) }))
            }
            .await,
        )
    }

    #[tool(
        description = "Check an APK, AAB or IPA (full path on this computer) against size limits: total, dex, native code and resources in megabytes (compressed), method count, and growth in percent over a baseline build. Returns what is over.",
        title = "Check a package against size limits",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn package_budget(&self, Parameters(a): Parameters<args::PackageBudget>) -> CallToolResult {
        reply(
            async {
                let file = package_path(&a.file)?;
                let package = emulabzip_audit::package::analyze_file(&file).map_err(|e| e.to_string())?;
                let baseline = match &a.baseline {
                    Some(b) => {
                        Some(emulabzip_audit::package::analyze_file(&package_path(b)?).map_err(|e| e.to_string())?)
                    }
                    None => None,
                };
                let limits = emulabzip_audit::package::Limits {
                    total_mb: a.total_mb,
                    dex_mb: a.dex_mb,
                    native_mb: a.native_mb,
                    resources_mb: a.resources_mb,
                    methods: a.methods,
                    growth_percent: a.growth_percent,
                };
                let breaches = emulabzip_audit::package::check(&package, &limits, baseline.as_ref());
                json(&serde_json::json!({ "within": breaches.is_empty(), "breaches": breaches }))
            }
            .await,
        )
    }
}

/// The folder visual baselines are kept in.
fn visual_dir() -> Result<std::path::PathBuf, String> {
    let config = emulabzip_session::Config::find(&std::env::current_dir().map_err(|e| e.to_string())?)
        .map(|p| emulabzip_session::Config::load(&p))
        .transpose()
        .map_err(|e| e.to_string())?
        .and_then(|c| c.visual)
        .unwrap_or_default();
    Ok(std::path::PathBuf::from(config.baseline_dir.as_deref().unwrap_or("visual")))
}
