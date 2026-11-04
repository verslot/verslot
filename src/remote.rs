use std::io::Write;
use std::time::{Duration, Instant};
use std::{fmt, str::FromStr};

use ureq::Agent;

use crate::distribution::NodePlatform;
use crate::download;
use crate::logger;
use crate::progress::Progress;
use crate::target::{Target, Tool, Version};

const INDEX_URL: &str = "https://nodejs.org/dist/index.tab";
const INDEX_LIMIT: u64 = 4 * 1024 * 1024;
const QUERY_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub(crate) enum RemoteQuery {
    All(Tool),
    Prefix(Tool, String),
    Lts,
}

impl FromStr for RemoteQuery {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input == "node@lts" {
            return Ok(Self::Lts);
        }
        let invalid = || {
            "invalid remote query: expected node, yarn, <tool>@<numeric-prefix>, or node@lts; use one to three ASCII decimal components without leading zeros, each within 0..=4294967295".to_owned()
        };
        let (tool, prefix) = input
            .split_once('@')
            .map_or((input, None), |(tool, prefix)| (tool, Some(prefix)));
        let tool = match tool {
            "node" => Tool::Node,
            "yarn" => Tool::Yarn,
            _ => return Err(invalid()),
        };
        let Some(prefix) = prefix else {
            return Ok(Self::All(tool));
        };
        let parts: Vec<_> = prefix.split('.').collect();
        if !(1..=3).contains(&parts.len())
            || parts.iter().any(|part| {
                part.is_empty()
                    || !part.bytes().all(|byte| byte.is_ascii_digit())
                    || (part.len() > 1 && part.starts_with('0'))
                    || part.parse::<u32>().is_err()
            })
        {
            return Err(invalid());
        }
        Ok(Self::Prefix(tool, prefix.to_owned()))
    }
}

impl fmt::Display for RemoteQuery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All(tool) => write!(formatter, "{tool}"),
            Self::Prefix(tool, prefix) => write!(formatter, "{tool}@{prefix}"),
            Self::Lts => formatter.write_str("node@lts"),
        }
    }
}

pub(crate) fn list(query: &RemoteQuery) -> Result<(), String> {
    if matches!(
        query,
        RemoteQuery::All(Tool::Yarn) | RemoteQuery::Prefix(Tool::Yarn, _)
    ) {
        let mut progress = Progress::stderr();
        progress.stage("Fetching versions");
        let result = crate::yarn::available_versions();
        progress.finish();
        let mut output = std::io::stdout().lock();
        for version in result? {
            if let RemoteQuery::Prefix(_, prefix) = query
                && !version.to_string().starts_with(prefix)
            {
                continue;
            }
            logger::write_plain_line(&mut output, &format!("v{version}"))
                .map_err(|error| format!("write release list: {error}"))?;
        }
        return Ok(());
    }
    let platform = NodePlatform::current()?;
    list_with_progress(
        query,
        platform.index_file,
        &download::download_agent(),
        INDEX_URL,
        Instant::now() + QUERY_TIMEOUT,
        &mut std::io::stdout().lock(),
        &mut Progress::stderr(),
    )
}

pub(crate) fn available_versions_for(tool: Tool) -> Result<Vec<Version>, String> {
    if tool == Tool::Yarn {
        return crate::yarn::available_versions();
    }
    fetch_versions(
        &RemoteQuery::All(Tool::Node),
        NodePlatform::current()?.index_file,
        &download::download_agent(),
        INDEX_URL,
        Instant::now() + QUERY_TIMEOUT,
    )
}

fn list_with_progress(
    query: &RemoteQuery,
    index_file: &str,
    agent: &Agent,
    url: &str,
    deadline: Instant,
    output: &mut impl Write,
    progress: &mut Progress<'_>,
) -> Result<(), String> {
    progress.stage("Fetching versions");
    let result = fetch_versions(query, index_file, agent, url, deadline);
    progress.finish();
    for version in result? {
        logger::write_plain_line(output, &format!("v{version}"))
            .map_err(|error| format!("write release list: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
fn list_with(
    query: &RemoteQuery,
    index_file: &str,
    agent: &Agent,
    url: &str,
    deadline: Instant,
    output: &mut impl Write,
) -> Result<(), String> {
    list_with_progress(
        query,
        index_file,
        agent,
        url,
        deadline,
        output,
        &mut Progress::Silent,
    )
}

pub(crate) fn fetch_versions(
    query: &RemoteQuery,
    index_file: &str,
    agent: &Agent,
    url: &str,
    deadline: Instant,
) -> Result<Vec<Version>, String> {
    let mut response = download::fetch_response(agent, url, deadline)?;
    let mut bytes = Vec::new();
    download::copy_bounded(
        &mut response.body_mut().as_reader(),
        &mut bytes,
        INDEX_LIMIT,
        deadline,
    )
    .map_err(|error| format!("read release index: {error}"))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("read release index: invalid UTF-8: {error}"))?;
    let mut lines = text.lines();
    let header: Vec<_> = lines
        .next()
        .ok_or("read release index: missing header")?
        .split('\t')
        .collect();
    let column = |name: &str| {
        let mut matches = header
            .iter()
            .enumerate()
            .filter(|(_, field)| **field == name);
        match (matches.next(), matches.next()) {
            (Some((index, _)), None) => Ok(index),
            _ => Err(format!("read release index: expected one {name} column")),
        }
    };
    let version_column = column("version")?;
    let files_column = column("files")?;
    let lts_column = column("lts")?;
    let mut versions = Vec::new();
    for (index, line) in lines.enumerate() {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != header.len() {
            return Err(format!("read release index: malformed row {}", index + 2));
        }
        let valid_files = fields[files_column].split(',').all(|file| {
            !file.is_empty()
                && file
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        });
        let lts = fields[lts_column];
        let valid_lts =
            lts == "-" || (!lts.is_empty() && lts.bytes().all(|byte| byte.is_ascii_alphabetic()));
        if !valid_files || !valid_lts {
            return Err(format!(
                "read release index: invalid release metadata in row {}",
                index + 2
            ));
        }
        let Some(version) = fields[version_column].strip_prefix('v') else {
            continue;
        };
        let Ok(target) = format!("node@{version}").parse::<Target>() else {
            continue;
        };
        if !fields[files_column]
            .split(',')
            .any(|file| file == index_file)
        {
            continue;
        }
        let matches = match query {
            RemoteQuery::All(_) => true,
            RemoteQuery::Prefix(_, prefix) => version.starts_with(prefix),
            RemoteQuery::Lts => fields[lts_column] != "-",
        };
        if !matches {
            continue;
        }
        versions.push(target.version);
    }
    versions.sort_unstable_by_key(|version| (version.major, version.minor, version.patch));
    versions.dedup();
    Ok(versions)
}

#[cfg(test)]
mod tests;
