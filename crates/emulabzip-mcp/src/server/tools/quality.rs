//! The quality tools.

use crate::server::*;

#[tool_router(router = quality_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "The views on a device's screen as a tree: class, text, label, id, size and position, and whether each can be tapped. Node numbers can be quoted back to the user. iOS needs Simulator.app open and the Accessibility permission."
    )]
    async fn get_layout(&self, Parameters(a): Parameters<args::Layout>) -> CallToolResult {
        reply(self.layout_inner(a).await)
    }

    #[tool(
        description = "Check the screen in front for accessibility problems: tappable views without a name, touch targets under 48 dp (Android) or 44 pt (iOS), text with too little contrast, and neighbours that sound the same. Each finding names the view, the WCAG criterion and a fix."
    )]
    async fn audit_accessibility(&self, Parameters(a): Parameters<args::Device>) -> CallToolResult {
        reply(
            async {
                let snapshot = self.lab.layout(&self.target(&a.device).await?).await.map_err(|e| e.to_string())?;
                json(&emulabzip_core::a11y::audit(&snapshot))
            }
            .await,
        )
    }

    #[tool(
        description = "Audit an installed app for security and privacy problems that are not about speed: secrets and personal data in the log, preferences and databases; the debuggable and allowBackup flags; components other apps can reach; App Transport Security exceptions and missing privacy manifest domains on iOS. Evidence is cut down so secrets are never repeated."
    )]
    async fn audit_security(&self, Parameters(a): Parameters<args::SecurityAudit>) -> CallToolResult {
        reply(
            async {
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                let mut what = emulabzip_audit::collect::Sources::default();
                match a.only.as_deref() {
                    None | Some([]) => what = emulabzip_audit::collect::Sources::all(None),
                    Some(areas) => {
                        for area in areas {
                            match area.as_str() {
                                "manifest" => what.manifest = true,
                                "logs" => what.logs = true,
                                "storage" => what.storage = true,
                                other => {
                                    return Err(format!("`{other}` is not an area: use manifest, logs or storage"));
                                }
                            }
                        }
                    }
                }
                let target = self.target(&a.device).await?;
                let input = emulabzip_audit::collect::collect(&self.lab, &target, &a.app, what).await;
                json(&emulabzip_audit::security::audit(&input))
            }
            .await,
        )
    }

    #[tool(
        description = "Break an APK, AAB or IPA file down by kind (dex, native, resources, assets), folder, ABI and dex file, with its biggest files and method count. Reads the file on this computer; nothing touches a device."
    )]
    async fn analyze_package(&self, Parameters(a): Parameters<args::Package>) -> CallToolResult {
        reply(
            async {
                let file = package_path(&a.file)?;
                let p = emulabzip_audit::package::analyze_file(&file).map_err(|e| e.to_string())?;
                let top = a.top.unwrap_or(15).clamp(1, 100);
                let mut value = serde_json::to_value(&p).map_err(|e| e.to_string())?;
                if let Some(o) = value.as_object_mut() {
                    o.remove("entries");
                    o.insert("largest".into(), serde_json::to_value(p.largest(top)).map_err(|e| e.to_string())?);
                    o.insert("methods".into(), p.methods().into());
                }
                json(&value)
            }
            .await,
        )
    }

    #[tool(
        description = "What changed in size between two builds (APK, AAB or IPA): the total, each kind and folder, and the biggest files added, removed or changed."
    )]
    async fn diff_packages(&self, Parameters(a): Parameters<args::PackageDiff>) -> CallToolResult {
        reply(
            async {
                let (before, after) = (package_path(&a.before)?, package_path(&a.after)?);
                let pa = emulabzip_audit::package::analyze_file(&before).map_err(|e| e.to_string())?;
                let pb = emulabzip_audit::package::analyze_file(&after).map_err(|e| e.to_string())?;
                let mut d = emulabzip_audit::package::diff(&pa, &pb);
                d.files.truncate(a.top.unwrap_or(15).clamp(1, 100));
                json(&d)
            }
            .await,
        )
    }

    #[tool(
        description = "Check the screen for text that does not fit: cut off, pushed past the edge of the screen, clipped by its container, or (with rtl) not mirrored for a right-to-left language. Pair it with control_locale (en-XA, ar-XB) and control_font_scale."
    )]
    async fn check_text(&self, Parameters(a): Parameters<args::CheckText>) -> CallToolResult {
        reply(
            async {
                let snapshot = self.lab.layout(&self.target(&a.device).await?).await.map_err(|e| e.to_string())?;
                json(&emulabzip_core::i18n::check(
                    &snapshot,
                    emulabzip_core::i18n::Options { rtl: a.rtl.unwrap_or(false) },
                ))
            }
            .await,
        )
    }

    #[tool(
        description = "Compare the screen with a baseline made by `emulabzip visual capture`: returns how much changed and where, and a picture with the changes in red. Baselines are looked for in the `visual` folder of the project (or `[visual] baseline_dir` of emulabzip.toml)."
    )]
    async fn visual_compare(&self, Parameters(a): Parameters<args::VisualCompare>) -> CallToolResult {
        let made = async {
            if !emulabzip_audit::visual::is_baseline_name(&a.baseline) {
                return Err(format!("`{}` is not a baseline name: use letters, digits, . _ and -", a.baseline));
            }
            let config = emulabzip_session::Config::find(&std::env::current_dir().map_err(|e| e.to_string())?)
                .map(|p| emulabzip_session::Config::load(&p))
                .transpose()
                .map_err(|e| e.to_string())?
                .and_then(|c| c.visual)
                .unwrap_or_default();
            let dir = std::path::PathBuf::from(config.baseline_dir.as_deref().unwrap_or("visual"));
            let baseline = emulabzip_audit::visual::load_baseline(&dir, &a.baseline).map_err(|e| e.to_string())?;
            let shot = self.lab.screenshot(&self.target(&a.device).await?).await.map_err(|e| e.to_string())?;
            let options = emulabzip_audit::visual::Options {
                tolerance: config.tolerance,
                ignore: config
                    .ignore
                    .iter()
                    .map(|r| emulabzip_audit::visual::Region { x: r.x, y: r.y, width: r.width, height: r.height })
                    .collect(),
            };
            let done = emulabzip_audit::visual::compare(&baseline, &shot, &options).map_err(|e| e.to_string())?;
            Ok((done, config.threshold_percent))
        }
        .await;
        match made {
            Ok((done, threshold)) => {
                let text = serde_json::json!({
                    "comparison": done.comparison,
                    "threshold": threshold,
                    "exceeds": done.comparison.exceeds(threshold),
                });
                let mut blocks = vec![ContentBlock::text(text.to_string())];
                if done.comparison.changed > 0 {
                    blocks.push(ContentBlock::image(
                        base64::engine::general_purpose::STANDARD.encode(done.diff),
                        "image/png",
                    ));
                }
                CallToolResult::success(blocks)
            }
            Err(message) => reply(Err(message)),
        }
    }

    #[tool(
        description = "Show the screen as someone with colour blindness sees it (protanopia, deuteranopia, tritanopia or achromatopsia): returns the findings of text that was readable normally and is not any more, and the changed screenshot."
    )]
    async fn simulate_color_blindness(&self, Parameters(a): Parameters<args::ColorBlindness>) -> CallToolResult {
        let made = async {
            let vision = emulabzip_core::Vision::parse(&a.vision).ok_or_else(|| {
                format!(
                    "`{}` is not a kind of colour blindness: use protanopia, deuteranopia, tritanopia or achromatopsia",
                    a.vision
                )
            })?;
            let snapshot = self.lab.layout(&self.target(&a.device).await?).await.map_err(|e| e.to_string())?;
            emulabzip_core::vision::simulate(&snapshot, vision).map_err(|e| e.to_string())
        }
        .await;
        match made {
            Ok(sim) => match json(&sim.audit) {
                Ok(text) => CallToolResult::success(vec![
                    ContentBlock::text(text),
                    ContentBlock::image(base64::engine::general_purpose::STANDARD.encode(sim.png), "image/png"),
                ]),
                Err(message) => reply(Err(message)),
            },
            Err(message) => reply(Err(message)),
        }
    }
}
