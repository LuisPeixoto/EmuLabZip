//! Links that open a new issue with the summary already filled in. Nothing is sent: the person reads it in their
//! browser and decides.

use std::fmt;

/// Where an issue goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tracker {
    GitHub { repo: String },
    GitLab { host: String, project: String },
    Jira { site: String, project_id: String, issue_type_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadTracker(pub String);

impl fmt::Display for BadTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BadTracker {}

const HELP: &str = "use github:owner/repo, gitlab:[host/]group/project or jira:site/PROJECTID/ISSUETYPEID";

fn part_ok(p: &str) -> bool {
    !p.is_empty()
        && p != "."
        && p != ".."
        && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

impl Tracker {
    /// `github:owner/repo`, `gitlab:group/project` (gitlab.com), `gitlab:host/group/project` (a host has a dot) or
    /// `jira:site/PROJECTID/ISSUETYPEID`.
    pub fn parse(spec: &str) -> Result<Tracker, BadTracker> {
        let bad = |why: &str| BadTracker(format!("'{spec}': {why}; {HELP}"));
        let (kind, rest) = spec.split_once(':').ok_or_else(|| bad("no tracker named"))?;
        let parts: Vec<&str> = rest.trim_matches('/').split('/').collect();
        match kind {
            "github" => match parts.as_slice() {
                [owner, repo] if part_ok(owner) && part_ok(repo) => {
                    Ok(Tracker::GitHub { repo: format!("{owner}/{repo}") })
                }
                _ => Err(bad("expected owner/repo")),
            },
            "gitlab" => {
                let (host, path) = if parts.first().is_some_and(|p| p.contains('.')) {
                    (parts[0], &parts[1..])
                } else {
                    ("gitlab.com", &parts[..])
                };
                if path.len() < 2 || !path.iter().all(|p| part_ok(p)) || !part_ok(host) {
                    return Err(bad("expected [host/]group/project"));
                }
                Ok(Tracker::GitLab { host: host.to_string(), project: path.join("/") })
            }
            "jira" => match parts.as_slice() {
                [site, pid, ty]
                    if part_ok(site)
                        && pid.chars().all(|c| c.is_ascii_digit())
                        && !pid.is_empty()
                        && ty.chars().all(|c| c.is_ascii_digit())
                        && !ty.is_empty() =>
                {
                    Ok(Tracker::Jira {
                        site: site.to_string(),
                        project_id: pid.to_string(),
                        issue_type_id: ty.to_string(),
                    })
                }
                _ => Err(bad("expected site/PROJECTID/ISSUETYPEID, the ids being numbers")),
            },
            other => Err(bad(&format!("unknown tracker '{other}'"))),
        }
    }
}

/// Percent-encodes everything but the unreserved characters.
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Browsers and trackers refuse very long links, so the whole URL stays under this.
pub const MAX_URL: usize = 6000;

fn cut_to(body: &str, room: usize) -> String {
    let note = "\n\n… (cut; see the attached report)";
    // Shorten the text until it and the note fit once encoded.
    let mut text: String = body.to_string();
    while !text.is_empty() && encode(&format!("{text}{note}")).len() > room {
        let keep = text.chars().count() * 9 / 10;
        text = text.chars().take(keep).collect();
    }
    format!("{}{note}", text.trim_end())
}

/// The link that opens a new issue with a title and a body.
pub fn issue_url(tracker: &Tracker, title: &str, body: &str) -> String {
    let (base, title_key, body_key, extra) = match tracker {
        Tracker::GitHub { repo } => (format!("https://github.com/{repo}/issues/new?"), "title", "body", String::new()),
        Tracker::GitLab { host, project } => (
            format!("https://{host}/{project}/-/issues/new?"),
            "issue%5Btitle%5D",
            "issue%5Bdescription%5D",
            String::new(),
        ),
        Tracker::Jira { site, project_id, issue_type_id } => (
            format!("https://{site}/secure/CreateIssueDetails!init.jspa?"),
            "summary",
            "description",
            format!("pid={project_id}&issuetype={issue_type_id}&"),
        ),
    };
    let head = format!("{base}{extra}{title_key}={}&{body_key}=", encode(title));
    let room = MAX_URL.saturating_sub(head.len());
    let body = if encode(body).len() > room { cut_to(body, room) } else { body.to_string() };
    format!("{head}{}", encode(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_three_trackers() {
        assert_eq!(
            Tracker::parse("github:LuisPeixoto/EmuLabZip").unwrap(),
            Tracker::GitHub { repo: "LuisPeixoto/EmuLabZip".into() }
        );
        assert_eq!(
            Tracker::parse("gitlab:group/sub/proj").unwrap(),
            Tracker::GitLab { host: "gitlab.com".into(), project: "group/sub/proj".into() }
        );
        assert_eq!(
            Tracker::parse("gitlab:git.corp.io/team/app").unwrap(),
            Tracker::GitLab { host: "git.corp.io".into(), project: "team/app".into() }
        );
        assert_eq!(
            Tracker::parse("jira:acme.atlassian.net/10001/10004").unwrap(),
            Tracker::Jira {
                site: "acme.atlassian.net".into(),
                project_id: "10001".into(),
                issue_type_id: "10004".into()
            }
        );
    }

    #[test]
    fn refuses_what_it_cannot_use() {
        for bad in [
            "",
            "github",
            "github:one",
            "github:a/b/c",
            "gitlab:x",
            "jira:site/ABC/1",
            "jira:site/1",
            "svn:a/b",
            "github:a b/c",
            "github:../x",
        ] {
            assert!(Tracker::parse(bad).is_err(), "{bad}");
        }
        assert!(Tracker::parse("github:one").unwrap_err().to_string().contains("github:owner/repo"));
    }

    #[test]
    fn builds_the_links() {
        let gh = issue_url(&Tracker::parse("github:a/b").unwrap(), "Crash & burn", "line 1\nline 2");
        assert_eq!(gh, "https://github.com/a/b/issues/new?title=Crash%20%26%20burn&body=line%201%0Aline%202");
        let gl = issue_url(&Tracker::parse("gitlab:g/p").unwrap(), "t", "b");
        assert_eq!(gl, "https://gitlab.com/g/p/-/issues/new?issue%5Btitle%5D=t&issue%5Bdescription%5D=b");
        let ji = issue_url(&Tracker::parse("jira:s.atlassian.net/1/2").unwrap(), "t", "b");
        assert_eq!(
            ji,
            "https://s.atlassian.net/secure/CreateIssueDetails!init.jspa?pid=1&issuetype=2&summary=t&description=b"
        );
    }

    #[test]
    fn a_long_body_is_cut_to_fit_and_still_valid() {
        let body = "áé ".repeat(5000);
        let url = issue_url(&Tracker::parse("github:a/b").unwrap(), "t", &body);
        assert!(url.len() <= MAX_URL, "{}", url.len());
        assert!(url.ends_with("%0A%0A%E2%80%A6%20%28cut%3B%20see%20the%20attached%20report%29"));
        assert!(!url.contains(' ') && !url.contains('\n'));
    }

    #[test]
    fn encodes_utf8() {
        assert_eq!(encode("ação"), "a%C3%A7%C3%A3o");
        assert_eq!(encode("a-b_c.d~"), "a-b_c.d~");
    }
}
