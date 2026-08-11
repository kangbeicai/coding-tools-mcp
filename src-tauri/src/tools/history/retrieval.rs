use serde_json::{json, Value};

use crate::tools::context::ToolContext;
use crate::tools::workspace::{tool_ok, WorkspaceResult};

use super::model::SearchHit;
use super::{history_error, resolve_dir, state, storage};

const DEFAULT_SEARCH_LIMIT: usize = 10;
const MAX_SEARCH_LIMIT: usize = 50;
const DEFAULT_READ_MAX_BYTES: usize = 32 * 1024;
const MAX_READ_MAX_BYTES: usize = 64 * 1024;

pub(super) fn search(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    let history_dir = resolve_dir(ctx, args)?;
    let report = storage::scan(&ctx.workspace, &history_dir)?;
    let manifest = current_manifest(&history_dir, &report);
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let tokens = state::tokenize(query);
    let limit = bounded_usize(args, "limit", DEFAULT_SEARCH_LIMIT, MAX_SEARCH_LIMIT)?;
    let cursor = bounded_usize(args, "cursor", 0, usize::MAX)?;
    let mut hits = collect_hits(&report, &manifest, &tokens);
    sort_hits(&mut hits);

    let total_matches = hits.len();
    let end = cursor.saturating_add(limit).min(total_matches);
    let page = if cursor >= total_matches {
        Vec::new()
    } else {
        hits.drain(cursor..end).collect()
    };
    Ok(tool_ok(json!({
        "query": query,
        "history_count": report.documents.len(),
        "total_matches": total_matches,
        "cursor": cursor,
        "limit": limit,
        "next_cursor": (end < total_matches).then_some(end),
        "results": page
    })))
}

pub(super) fn read(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    let history_dir = resolve_dir(ctx, args)?;
    let report = storage::scan(&ctx.workspace, &history_dir)?;
    let document = select_document(&report, args)?;
    let content_hash = storage::sha256(document.content.as_bytes());
    verify_expected_hash(args, &content_hash)?;

    let cursor = bounded_usize(args, "cursor", 0, document.content.len())?;
    validate_cursor(&document.content, cursor)?;
    let requested = bounded_usize(
        args,
        "max_bytes",
        DEFAULT_READ_MAX_BYTES,
        MAX_READ_MAX_BYTES,
    )?;
    let end = page_end(&document.content, cursor, requested);

    Ok(tool_ok(json!({
        "number": document.number,
        "path": document.path,
        "content": &document.content[cursor..end],
        "cursor": cursor,
        "next_cursor": (end < document.content.len()).then_some(end),
        "total_bytes": document.content.len(),
        "content_hash": content_hash
    })))
}

fn current_manifest(
    history_dir: &std::path::Path,
    report: &super::model::ScanReport,
) -> super::model::MemoryManifest {
    let rebuilt = state::build_manifest(report);
    storage::read_manifest(history_dir)
        .ok()
        .flatten()
        .filter(|manifest| manifest.archive_revision == rebuilt.archive_revision)
        .unwrap_or(rebuilt)
}

fn collect_hits(
    report: &super::model::ScanReport,
    manifest: &super::model::MemoryManifest,
    tokens: &[String],
) -> Vec<SearchHit> {
    manifest
        .entries
        .iter()
        .filter_map(|entry| {
            let document = report
                .documents
                .iter()
                .find(|document| document.number == entry.number)?;
            let score = search_score(entry, &document.content, tokens);
            if !tokens.is_empty() && score == 0 {
                return None;
            }
            Some(SearchHit {
                number: entry.number,
                path: entry.path.clone(),
                title: entry.title.clone(),
                updated_at: entry.updated_at.clone(),
                sha256: entry.sha256.clone(),
                score,
                snippet: search_snippet(&document.content, tokens),
            })
        })
        .collect()
}

fn sort_hits(hits: &mut [SearchHit]) {
    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| right.number.cmp(&left.number))
    });
}

fn select_document<'a>(
    report: &'a super::model::ScanReport,
    args: &Value,
) -> WorkspaceResult<&'a super::model::HistoryDocument> {
    let document = if let Some(number) = args.get("number").and_then(Value::as_u64) {
        report
            .documents
            .iter()
            .find(|document| document.number == number)
    } else if let Some(path) = args.get("path").and_then(Value::as_str) {
        report
            .documents
            .iter()
            .find(|document| document.path == path)
    } else {
        None
    };
    document.ok_or_else(|| {
        history_error(
            "HISTORY_READ_NOT_FOUND",
            "Pass an existing archive number or a manifest-returned relative path.",
            "not_found",
            false,
            json!({}),
        )
    })
}

fn verify_expected_hash(args: &Value, content_hash: &str) -> WorkspaceResult<()> {
    let Some(expected_hash) = args.get("expected_hash").and_then(Value::as_str) else {
        return Ok(());
    };
    if expected_hash == content_hash {
        return Ok(());
    }
    Err(history_error(
        "HISTORY_ARCHIVE_CHANGED",
        "The archive changed since the previous page; restart the read with the new hash.",
        "conflict",
        true,
        json!({"expected_hash": expected_hash, "content_hash": content_hash}),
    ))
}

fn validate_cursor(content: &str, cursor: usize) -> WorkspaceResult<()> {
    if cursor <= content.len() && content.is_char_boundary(cursor) {
        return Ok(());
    }
    Err(history_error(
        "HISTORY_CURSOR_INVALID",
        "cursor must be a UTF-8 character boundary inside the archive.",
        "validation",
        false,
        json!({"cursor": cursor, "total_bytes": content.len()}),
    ))
}

fn page_end(content: &str, cursor: usize, requested: usize) -> usize {
    let mut end = cursor.saturating_add(requested).min(content.len());
    while end > cursor && !content.is_char_boundary(end) {
        end -= 1;
    }
    if end == cursor && end < content.len() {
        end += content[cursor..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(0);
    }
    end
}

fn search_score(entry: &super::model::ManifestEntry, content: &str, tokens: &[String]) -> u64 {
    if tokens.is_empty() {
        return 1;
    }
    let title = entry.title.to_lowercase();
    let keywords = entry.keywords.join(" ").to_lowercase();
    let content = content.to_lowercase();
    tokens.iter().fold(0, |score, token| {
        score
            + u64::from(title.contains(token)) * 16
            + u64::from(keywords.contains(token)) * 10
            + u64::from(content.contains(token)) * 4
    })
}

fn search_snippet(content: &str, tokens: &[String]) -> String {
    if tokens.is_empty() {
        return state::truncate_text(content.trim(), 280);
    }
    let (normalized, source_offsets) = normalized_search_text(content);
    let start = tokens
        .iter()
        .filter_map(|token| normalized.find(token))
        .filter_map(|offset| source_offsets.get(offset).copied())
        .min()
        .unwrap_or(0);
    let prefix = &content[..start];
    let start = prefix
        .char_indices()
        .rev()
        .nth(80)
        .map(|(index, _)| index)
        .unwrap_or(0);
    state::truncate_text(content[start..].trim(), 280)
}

fn normalized_search_text(content: &str) -> (String, Vec<usize>) {
    let mut normalized = String::with_capacity(content.len());
    let mut source_offsets = Vec::with_capacity(content.len());
    for (source_offset, character) in content.char_indices() {
        for lower in character.to_lowercase() {
            normalized.push(lower);
            source_offsets.extend(std::iter::repeat_n(source_offset, lower.len_utf8()));
        }
    }
    (normalized, source_offsets)
}

fn bounded_usize(
    args: &Value,
    name: &str,
    default: usize,
    maximum: usize,
) -> WorkspaceResult<usize> {
    let Some(value) = args.get(name) else {
        return Ok(default);
    };
    let value = value.as_u64().ok_or_else(|| invalid_bound(name, maximum))?;
    let value = usize::try_from(value).map_err(|_| invalid_bound(name, maximum))?;
    if value > maximum || matches!(name, "limit" | "max_bytes") && value == 0 {
        return Err(invalid_bound(name, maximum));
    }
    Ok(value)
}

fn invalid_bound(name: &str, maximum: usize) -> crate::tools::workspace::WorkspaceError {
    history_error(
        "HISTORY_CURSOR_INVALID",
        &format!("{name} is outside the allowed range."),
        "validation",
        false,
        json!({"argument": name, "maximum": maximum}),
    )
}
