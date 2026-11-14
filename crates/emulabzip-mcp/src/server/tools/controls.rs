//! The controls tools.

use crate::server::*;

#[tool_router(router = controls_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "What a device supports changing, its appearance, and the permissions of an app when `app` is given."
    )]
    async fn control_info(&self, Parameters(a): Parameters<args::App>) -> CallToolResult {
        reply(
            async {
                json(
                    &self
                        .lab
                        .controls(&self.target(&a.device).await?, Some(&a.app))
                        .await
                        .map_err(|e| e.to_string())?,
                )
            }
            .await,
        )
    }

    #[tool(
        description = "Change the device's location (emulators and simulators). Without lat and lon, go back to the default location."
    )]
    async fn control_location(&self, Parameters(a): Parameters<args::Location>) -> CallToolResult {
        reply(
            async {
                match (a.lat, a.lon) {
                    (Some(lat), Some(lon)) => {
                        self.control(&a.device, Control::Location { lat, lon }, "Location set").await
                    }
                    (None, None) => self.control(&a.device, Control::ClearLocation, "Location reset").await,
                    _ => Err("Give both lat and lon, or neither".into()),
                }
            }
            .await,
        )
    }

    #[tool(description = "Switch the device between light and dark mode.")]
    async fn control_appearance(&self, Parameters(a): Parameters<args::Appearance>) -> CallToolResult {
        reply(
            async {
                let appearance = match a.mode.as_str() {
                    "light" => Appearance::Light,
                    "dark" => Appearance::Dark,
                    other => return Err(format!("`{other}` is not a mode: use light or dark")),
                };
                self.control(&a.device, Control::Appearance { appearance }, &format!("Appearance set to {}", a.mode))
                    .await
            }
            .await,
        )
    }

    #[tool(
        description = "Show an app in another language or a pseudo-locale (en-XA for long text, ar-XB for right to left), to find text that does not fit; or put the language back. Android changes one app (Android 13+); iOS changes the whole simulator and apps pick it up when they start."
    )]
    async fn control_locale(&self, Parameters(a): Parameters<args::Locale>) -> CallToolResult {
        reply(
            async {
                match a.locale {
                    Some(locale) => {
                        let said = format!("Language set to {locale}");
                        self.control(&a.device, Control::Locale { locale, app: a.app }, &said).await
                    }
                    None => {
                        self.control(&a.device, Control::ResetLocale { app: a.app }, "Language back to normal").await
                    }
                }
            }
            .await,
        )
    }

    #[tool(
        description = "Make the device's text bigger or smaller, as a percentage of normal (100 is normal, 130 and 200 are common accessibility settings), to find text that does not fit. Set 100 to put it back."
    )]
    async fn control_font_scale(&self, Parameters(a): Parameters<args::FontScale>) -> CallToolResult {
        reply(
            self.control(
                &a.device,
                Control::FontScale { percent: a.percent },
                &format!("Text size set to {}%", a.percent),
            )
            .await,
        )
    }

    #[tool(
        description = "Change the screen size (pixels) and density (dpi) of an Android device, or put them back with reset."
    )]
    async fn control_display_size(&self, Parameters(a): Parameters<args::DisplaySize>) -> CallToolResult {
        reply(
            async {
                if a.reset.unwrap_or(false) {
                    return self.control(&a.device, Control::ResetDisplay, "Display back to normal").await;
                }
                self.control(
                    &a.device,
                    Control::Display { width: a.width, height: a.height, density: a.density },
                    "Display changed",
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Show a clean status bar (fixed time, battery and signal), for screenshots; or clear the override."
    )]
    async fn control_status_bar(&self, Parameters(a): Parameters<args::StatusBar>) -> CallToolResult {
        reply(
            async {
                if a.clear.unwrap_or(false) {
                    return self.control(&a.device, Control::ClearStatusBar, "Status bar back to normal").await;
                }
                let status_bar = StatusBar { time: a.time, battery: a.battery, wifi: a.wifi, cellular: a.cellular };
                self.control(&a.device, Control::StatusBar { status_bar }, "Status bar overridden").await
            }
            .await,
        )
    }

    #[tool(
        description = "Grant, revoke or reset one permission of an app (an Android permission name, or an iOS service such as photos)."
    )]
    async fn control_permission(&self, Parameters(a): Parameters<args::Permission>) -> CallToolResult {
        reply(
            async {
                let grant = grant_of(&a.action)?;
                let done = format!("{}: {}", a.permission, a.action);
                self.control(&a.device, Control::Permission { app: a.app, permission: a.permission, grant }, &done)
                    .await
            }
            .await,
        )
    }

    #[tool(description = "Post a notification (Android) or send a simulated push to an app (iOS).")]
    async fn control_notification(&self, Parameters(a): Parameters<args::Notification>) -> CallToolResult {
        reply(
            self.control(
                &a.device,
                Control::Notification {
                    app: a.app.unwrap_or_else(|| "com.android.shell".into()),
                    title: a.title,
                    body: a.body,
                    payload: a.payload,
                },
                "Notification sent",
            )
            .await,
        )
    }

    #[tool(
        description = "Open a web address or an app's own link on the device. Links that read local files or run script are refused."
    )]
    async fn open_url(&self, Parameters(a): Parameters<args::OpenUrl>) -> CallToolResult {
        reply(self.control(&a.device, Control::OpenUrl { url: a.url }, "Link opened").await)
    }

    #[tool(description = "Start an app on the device, by package name or bundle id, as the launcher would.")]
    async fn launch_app(&self, Parameters(a): Parameters<args::LaunchApp>) -> CallToolResult {
        reply(self.control(&a.device, Control::Launch { app: a.app }, "App started").await)
    }

    #[tool(
        description = "Touch a point of the screen of an Android device. Positions are pixels, as in a screenshot (get_layout lists where each view is). iOS has no public way to send touches."
    )]
    async fn tap(&self, Parameters(a): Parameters<args::Tap>) -> CallToolResult {
        reply(self.control(&a.device, Control::Tap { x: a.x, y: a.y }, "Tapped").await)
    }

    #[tool(
        description = "Drag a finger across the screen of an Android device, from one point to another in pixels. From a point to itself it is a long press."
    )]
    async fn swipe(&self, Parameters(a): Parameters<args::Swipe>) -> CallToolResult {
        reply(
            self.control(
                &a.device,
                Control::Swipe { x1: a.x1, y1: a.y1, x2: a.x2, y2: a.y2, duration_ms: a.duration_ms },
                "Swiped",
            )
            .await,
        )
    }

    #[tool(description = "Type text into the focused field of an Android device. Printable ASCII only.")]
    async fn input_text(&self, Parameters(a): Parameters<args::InputText>) -> CallToolResult {
        reply(self.control(&a.device, Control::Text { text: a.text }, "Typed").await)
    }

    #[tool(description = "Press a key on an Android device: back, home, enter, tab, delete, volume_up, app_switch…")]
    async fn press_key(&self, Parameters(a): Parameters<args::PressKey>) -> CallToolResult {
        reply(self.control(&a.device, Control::Key { key: a.key }, "Pressed").await)
    }

    #[tool(
        description = "Pretend the battery is at a level, charging or not (Android); or go back to the real battery."
    )]
    async fn control_battery(&self, Parameters(a): Parameters<args::Battery>) -> CallToolResult {
        reply(
            async {
                if a.reset.unwrap_or(false) {
                    return self.control(&a.device, Control::ResetBattery, "Battery back to normal").await;
                }
                let level = a.level.ok_or("Give a level, or set reset to true")?;
                self.control(
                    &a.device,
                    Control::Battery { level, charging: a.charging.unwrap_or(false) },
                    "Battery overridden",
                )
                .await
            }
            .await,
        )
    }
}
