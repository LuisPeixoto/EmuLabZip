//! Checking a deep link: reads the domain's file over HTTPS, asks the device what it decided, and says what is wrong.

use std::time::Duration;

use emulabzip_core::links::{self, Facts, Fetch, Finding, Level, Link, RemoteFile};
use emulabzip_core::{Control, Error, Lab, Platform, Result, Target};
use serde::Serialize;

use crate::fetch::fetch;

/// The most of a domain file that is read, and how long it may take.
const LIMIT: usize = 256 * 1024;
const WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub url: String,
    pub platform: Platform,
    pub app: Option<String>,
    /// The domain file that was read, when the link is a web link.
    pub file_url: Option<String>,
    pub findings: Vec<Finding>,
    /// The domains of the app and what the device decided about each (Android).
    pub domains: Vec<links::DomainState>,
    pub worst: Level,
}

/// Reads one file of a domain, without following redirects.
async fn read(url: &str) -> Fetch {
    match fetch(url, LIMIT, WAIT).await {
        Ok(f) => Ok(RemoteFile {
            url: url.to_string(),
            status: f.status,
            content_type: f.content_type,
            location: f.location,
            body: String::from_utf8_lossy(&f.body).into_owned(),
            truncated: f.truncated,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Checks `url` for the device. The link and the app are validated before anything is read or asked.
pub async fn check(lab: &Lab, target: &Target, url: &str, app: Option<&str>) -> Result<Report> {
    Control::OpenUrl { url: url.to_string() }.validate()?;
    if let Some(app) = app
        && !emulabzip_core::control::is_app_id(app)
    {
        return Err(Error::invalid("app", app));
    }
    let link = Link::parse(url).ok_or_else(|| Error::invalid("url", url))?;
    let (file_url, file) = match (&link.host, link.is_web()) {
        (Some(host), true) => {
            let file_url = links::domain_file_url(host, target.platform);
            let file = read(&file_url).await;
            (Some(file_url), Some(file))
        }
        _ => (None, None),
    };
    let facts = match lab.link_facts(target, app, url).await {
        Ok(facts) => facts,
        Err(e) => Facts { notes: vec![e.to_string()], ..Facts::default() },
    };
    let findings = links::evaluate(&link, target.platform, app, file.as_ref(), &facts);
    let domains = facts.app_links.map(|a| a.domains).unwrap_or_default();
    let worst = links::worst(&findings);
    Ok(Report {
        url: url.to_string(),
        platform: target.platform,
        app: app.map(str::to_string),
        file_url,
        findings,
        domains,
        worst,
    })
}

#[cfg(test)]
mod tests {
    use emulabzip_core::Platform;

    use super::*;

    #[tokio::test]
    async fn a_bad_link_or_app_is_refused_before_anything_is_read() {
        let lab = Lab::new();
        let target = Target { platform: Platform::Android, id: "x".into() };
        assert!(matches!(check(&lab, &target, "file:///etc/passwd", None).await, Err(Error::Invalid { .. })));
        assert!(matches!(check(&lab, &target, "https://a.example/x", Some("a b")).await, Err(Error::Invalid { .. })));
    }
}
