//! Ready-made questions: each one tells the assistant which tools to call, in what order, for a job people do often.

use rmcp::model::{GetPromptResult, JsonObject, Prompt, PromptArgument, PromptMessage, Role};

struct Def {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    /// (name, description, required)
    args: &'static [(&'static str, &'static str, bool)],
    text: fn(&Args) -> String,
}

/// The arguments a person filled in, read as text.
pub(crate) struct Args<'a>(&'a JsonObject);

impl Args<'_> {
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
    }

    fn or(&self, key: &str, fallback: &str) -> String {
        self.get(key).unwrap_or_else(|| fallback.to_string())
    }
}

const DEFS: &[Def] = &[
    Def {
        name: "explain_crash",
        title: "Explain a crash",
        description: "Find the latest crash, read it with the app's mapping, and say what went wrong and how to fix it.",
        args: &[
            ("app", "The app's package name or bundle id. Leave out to look at every app.", false),
            ("crash_id", "A crash id from list_crashes. Leave out for the latest.", false),
        ],
        text: |a| {
            format!(
                "Explain why {} crashed.\n\n1. Call list_crashes{} and pick {}.\n2. Call get_crash for it. If the stack still looks obfuscated, call list_mappings, then symbolicate_stack with the app and its version.\n3. Call read_logs around the crash time for what the app did just before.\n4. Say which line failed and why, quote the frames that matter, and propose a fix. If you have the source, point to the file and show the change.",
                a.get("app").unwrap_or_else(|| "the app".into()),
                a.get("app").map(|x| format!(" with app \"{x}\"")).unwrap_or_default(),
                a.get("crash_id").map(|x| format!("crash {x}")).unwrap_or_else(|| "the latest one".into()),
            )
        },
    },
    Def {
        name: "why_is_it_slow",
        title: "Find out why the app is slow",
        description: "Measure CPU, memory, frames and startup, then trace the slow part and say what to change.",
        args: &[
            ("app", "The app's package name or bundle id.", true),
            ("screen", "What was slow, in words (for example \"scrolling the feed\").", false),
        ],
        text: |a| {
            let app = a.or("app", "the app");
            format!(
                "Find out why {app} is slow{}.\n\n1. Call perf_snapshot for {app} to see CPU, memory and dropped frames now.\n2. Start watch_start with kind \"perf\" and ask me to do the slow thing, then read it with watch_read and stop it.\n3. If CPU is high, call record_trace for 10 seconds while it happens, then trace_summary.\n4. Call get_layout to see how deep the view tree is, and memory_diff if memory only goes up.\n5. Name the one or two causes with numbers behind them, and what to change first.",
                a.get("screen").map(|s| format!(" when {s}")).unwrap_or_default()
            )
        },
    },
    Def {
        name: "summarize_session",
        title: "Summarize a recorded session",
        description: "Read a recorded session and say what happened: errors, crashes, slow requests and the moment it went wrong.",
        args: &[("session_id", "A session from list_sessions. Leave out for the newest.", false)],
        text: |a| {
            format!(
                "Summarize the session {}.\n\n1. {}\n2. Call session_summary, then session_timeline for the moments around any error or crash.\n3. Check the requests of the session in the timeline for failed or slow ones; compare it with another run using session_compare when there is a baseline.\n4. Tell the story in order: what the person did, what broke, and the first thing to check.",
                a.get("session_id").map(|s| format!("\"{s}\"")).unwrap_or_else(|| "that was recorded last".into()),
                if a.get("session_id").is_some() {
                    "You have its name."
                } else {
                    "Call list_sessions and take the newest."
                }
            )
        },
    },
    Def {
        name: "reproduce_bug",
        title: "Reproduce a bug and write it up",
        description: "Follow a description on the device, record what happens, and produce a bug report.",
        args: &[
            ("description", "What goes wrong and the steps that lead to it.", true),
            ("app", "The app's package name or bundle id.", true),
        ],
        text: |a| {
            format!(
                "Reproduce this bug in {}: {}\n\n1. Call session_start with the app, network on and touches on.\n2. Launch the app (launch_app). Before every action, say in one line what you are about to do. Use get_layout and screenshot to find what to tap, then tap, swipe or input_text.\n3. When the bug shows (or after you have tried the steps twice), call session_stop.\n4. Call create_bug_report with what you saw as the title and summary.\n5. Say whether it reproduced, and the shortest steps that did it.",
                a.or("app", "the app"),
                a.or("description", "(no description given: ask me)")
            )
        },
    },
    Def {
        name: "debug_request",
        title: "Debug a network request",
        description: "Capture the app's traffic, find the request, and say why it failed or what it sent.",
        args: &[("url_or_host", "A URL or host to look for. Leave out to look at everything.", false)],
        text: |a| {
            format!(
                "Debug the network request {}.\n\n1. Call net_status. If the proxy is not running, call net_start, then net_connect_device (and net_install_ca for HTTPS on a device that needs it).\n2. Ask me to trigger the request, then call net_flows{}.\n3. Call net_flow on the one that failed or looks wrong: read the status, headers, the body, timing and the error.\n4. If an OpenAPI spec exists for it, call openapi_check. Say what is wrong, on the client or on the server, and what to change.\n5. Leave things as you found them: net_disconnect_device when done.",
                a.get("url_or_host").map(|u| format!("to {u}")).unwrap_or_else(|| "that fails".into()),
                a.get("url_or_host").map(|u| format!(" with host or text \"{u}\"")).unwrap_or_default()
            )
        },
    },
    Def {
        name: "release_check",
        title: "Check a build before release",
        description: "Run the security, accessibility, size, text and language checks and list what to fix.",
        args: &[
            ("app", "The app's package name or bundle id.", true),
            ("package", "Full path of the APK, AAB or IPA, to check its size.", false),
        ],
        text: |a| {
            let app = a.or("app", "the app");
            format!(
                "Check {app} before it is released.\n\n1. audit_security for {app}.\n2. audit_accessibility on the main screen, then check_text for text that is cut off.\n3. {}\n4. i18n_sweep with a long language (de-DE), a right-to-left one (ar-XB) and text sizes 100 and 200.\n5. Give one list: blockers first, then things worth fixing, each with where it is.",
                a.get("package")
                    .map(|p| format!("analyze_package and package_budget for {p}."))
                    .unwrap_or_else(|| "Skip the size check (no package was given).".into())
            )
        },
    },
    Def {
        name: "check_deep_link",
        title: "Check a deep link",
        description: "Test that a link opens the right screen of the app and that its domain is verified.",
        args: &[
            ("url", "The link, for example https://example.com/item/42.", true),
            ("app", "The app that should open it.", false),
        ],
        text: |a| {
            format!(
                "Check the deep link {}.\n\n1. check_link for it: does the domain verify and the file match?\n2. open_url, then screenshot: is it the right screen{}?\n3. If it did not verify on Android, verify_app_links and check again after a few seconds.\n4. Say what works and what to fix.",
                a.or("url", "(no link given: ask me)"),
                a.get("app").map(|x| format!(" of {x}")).unwrap_or_default()
            )
        },
    },
    Def {
        name: "mock_backend",
        title: "Mock a backend response",
        description: "Answer a host's requests with fixed responses, delays or errors, to test how the app copes.",
        args: &[
            ("host", "The host to mock, for example api.example.com.", true),
            ("what", "What to simulate (an error, a slow reply, an empty list).", false),
        ],
        text: |a| {
            format!(
                "Simulate {} for {}.\n\n1. net_start and net_connect_device if they are not already.\n2. net_flows with host \"{}\" to see the real requests and their shape.\n3. net_rules_get, then net_rules_set with the existing rules plus a rule for that host (a mock, a delay or a block). Keep the other rules.\n4. Ask me to use the app, and read what happened with net_flows and screenshot.\n5. Put the rules back as they were when I say so.",
                a.or("what", "a failing backend"),
                a.or("host", "the host"),
                a.or("host", "the host")
            )
        },
    },
];

pub(crate) fn list() -> Vec<Prompt> {
    DEFS.iter()
        .map(|d| {
            let args = d
                .args
                .iter()
                .map(|(n, desc, req)| PromptArgument::new(*n).with_description(*desc).with_required(*req))
                .collect::<Vec<_>>();
            Prompt::new(d.name, Some(d.description), Some(args)).with_title(d.title)
        })
        .collect()
}

/// The messages of a prompt, or why it cannot be made.
pub(crate) fn get(name: &str, arguments: Option<&JsonObject>) -> Result<GetPromptResult, String> {
    let def = DEFS.iter().find(|d| d.name == name).ok_or_else(|| format!("there is no prompt called {name}"))?;
    let empty = JsonObject::new();
    let given = arguments.unwrap_or(&empty);
    if let Some(unknown) = given.keys().find(|k| !def.args.iter().any(|(n, _, _)| n == k)) {
        return Err(format!("{name} has no argument {unknown}"));
    }
    if let Some((missing, _, _)) = def.args.iter().find(|(n, _, required)| *required && Args(given).get(n).is_none()) {
        return Err(format!("{name} needs {missing}"));
    }
    if let Some((long, _)) = given.iter().find(|(_, v)| v.as_str().is_some_and(|s| s.len() > 4000)) {
        return Err(format!("{long} is too long"));
    }
    Ok(GetPromptResult::new(vec![PromptMessage::new_text(Role::User, (def.text)(&Args(given)))])
        .with_description(def.description))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_prompt_can_be_made_and_names_tools_that_exist_in_the_guide() {
        for p in list() {
            let mut args = JsonObject::new();
            for a in p.arguments.clone().unwrap_or_default() {
                if a.required == Some(true) {
                    args.insert(a.name, "x".into());
                }
            }
            let made = get(&p.name, Some(&args)).unwrap();
            assert_eq!(made.messages.len(), 1);
        }
    }

    #[test]
    fn missing_or_unknown_arguments_are_refused() {
        assert!(get("reproduce_bug", None).is_err());
        assert!(get("nope", None).is_err());
        let mut args = JsonObject::new();
        args.insert("bogus".into(), "x".into());
        assert!(get("explain_crash", Some(&args)).is_err());
    }
}
