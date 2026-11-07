//! Reporting a bug in one go.
//!
//! A bug report needs the same things every time: what the screen looked like, what device and app, what the log said,
//! which requests failed and how the app crashed, readable. This gathers them into a [`BugReport`], then writes it as
//! one self-contained HTML file ([`html`]), as a short Markdown summary for an issue ([`markdown`]) and as a link that
//! opens a new issue with that summary filled in ([`issue`]). Nothing is sent anywhere: the person looks at the file,
//! and chooses what to do with it.

pub mod html;
pub mod issue;
pub mod markdown;
pub mod model;

pub use model::{
    BugReport, CrashSection, FailedRequest, LogLine, LogSection, Options, Strip, StripMark, VIDEO_FILE, VideoClip,
};
