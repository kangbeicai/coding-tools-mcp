use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use super::markdown;
use super::model::{
    HistoryDocument, ManifestEntry, MemoryManifest, MemoryReference, MemoryState, ScanReport,
};
use super::storage;

const STATE_ITEM_LIMIT: usize = 12;
const STATE_TEXT_LIMIT: usize = 512;
const STATE_FOCUS_LIMIT: usize = 2_048;
const STATE_REFERENCE_LIMIT: usize = 8;

pub(super) fn build_manifest(report: &ScanReport) -> MemoryManifest {
    let entries = report
        .documents
        .iter()
        .map(manifest_entry)
        .collect::<Vec<_>>();
    let mut digest = Sha256::new();
    for entry in &entries {
        digest.update(entry.number.to_le_bytes());
        digest.update(entry.sha256.as_bytes());
    }
    MemoryManifest {
        version: 2,
        archive_revision: format!("sha256:{:x}", digest.finalize()),
        entries,
    }
}

pub(super) fn build_state(
    report: &ScanReport,
    manifest: &MemoryManifest,
    current_number: Option<u64>,
    timestamp: &str,
    state_revision: u64,
) -> MemoryState {
    let current_document = current_document(report, current_number);
    let current_focus = focus_text(current_document);
    let (recent_changes, open_items) = bounded_progress(report);
    MemoryState {
        version: 2,
        state_revision,
        archive_revision: manifest.archive_revision.clone(),
        generated_at: timestamp.to_string(),
        current_session: current_reference(manifest, current_number),
        current_focus: truncate_text(&current_focus, STATE_FOCUS_LIMIT),
        recent_changes,
        open_items,
        references: recent_references(manifest, current_number),
    }
}

pub(super) fn tokenize(value: &str) -> Vec<String> {
    let mut tokens = value
        .split(|character: char| !character.is_alphanumeric())
        .map(|token| token.trim().to_lowercase())
        .filter(|token| token.chars().count() >= 2)
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.dedup();
    tokens
}

pub(super) fn truncate_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut text = value.chars().take(max_chars).collect::<String>();
    text.push_str("...");
    text
}

fn manifest_entry(document: &HistoryDocument) -> ManifestEntry {
    ManifestEntry {
        number: document.number,
        path: document.path.clone(),
        title: markdown::document_title(&document.content, document.number),
        created_at: document.created_at.clone().unwrap_or_default(),
        updated_at: document.updated_at.clone().unwrap_or_default(),
        bytes: document.content.len() as u64,
        sha256: storage::sha256(document.content.as_bytes()),
        keywords: keywords(document),
    }
}

fn current_document(report: &ScanReport, current_number: Option<u64>) -> Option<&HistoryDocument> {
    current_number.and_then(|number| {
        report
            .documents
            .iter()
            .find(|document| document.number == number)
    })
}

fn focus_text(document: Option<&HistoryDocument>) -> String {
    document
        .and_then(|document| latest_user_focus(&document.content))
        .or_else(|| document.map(|doc| markdown::document_title(&doc.content, doc.number)))
        .unwrap_or_else(|| "尚未记录当前焦点".to_string())
}

fn bounded_progress(report: &ScanReport) -> (Vec<String>, Vec<String>) {
    let mut recent_changes = Vec::new();
    let mut open_items = Vec::new();
    for document in report.documents.iter().rev() {
        let records = markdown::parse_checkpoint_records(&document.content);
        for record in latest_revisions(&records).into_iter().rev() {
            push_record_progress(record, &mut recent_changes, &mut open_items);
        }
    }
    (recent_changes, open_items)
}

fn push_record_progress(
    record: &super::model::CheckpointRecord,
    recent_changes: &mut Vec<String>,
    open_items: &mut Vec<String>,
) {
    push_bounded(
        recent_changes,
        record.files_changed.iter().map(String::as_str),
    );
    push_bounded(
        recent_changes,
        record.decisions.iter().map(String::as_str),
    );
    push_bounded(
        open_items,
        record.remaining_issues.iter().map(String::as_str),
    );
    push_bounded(
        open_items,
        record.next_actions.iter().map(String::as_str),
    );
}

fn current_reference(
    manifest: &MemoryManifest,
    current_number: Option<u64>,
) -> Option<MemoryReference> {
    current_number.and_then(|number| {
        manifest
            .entries
            .iter()
            .find(|entry| entry.number == number)
            .map(|entry| MemoryReference {
                number: entry.number,
                path: entry.path.clone(),
                reason: "当前 ChatGPT 会话档案".into(),
            })
    })
}

fn recent_references(
    manifest: &MemoryManifest,
    current_number: Option<u64>,
) -> Vec<MemoryReference> {
    manifest
        .entries
        .iter()
        .rev()
        .filter(|entry| Some(entry.number) != current_number)
        .take(STATE_REFERENCE_LIMIT)
        .map(|entry| MemoryReference {
            number: entry.number,
            path: entry.path.clone(),
            reason: "最近历史档案；可按需读取原文".into(),
        })
        .collect()
}

fn latest_user_focus(content: &str) -> Option<String> {
    let records = markdown::parse_checkpoint_records(content);
    latest_revisions(&records)
        .into_iter()
        .last()
        .and_then(record_focus)
        .or_else(|| {
            markdown::parse_initial_input_records(content)
                .into_iter()
                .max_by_key(|record| record.revision)
                .map(|record| record.raw_user_input)
        })
}

fn record_focus(record: &super::model::CheckpointRecord) -> Option<String> {
    if !record.raw_user_input.trim().is_empty() {
        Some(record.raw_user_input.clone())
    } else if !record.user_intent.trim().is_empty() {
        Some(record.user_intent.clone())
    } else {
        None
    }
}

fn latest_revisions(
    records: &[super::model::CheckpointRecord],
) -> Vec<&super::model::CheckpointRecord> {
    let mut seen = BTreeSet::new();
    let mut latest = Vec::new();
    for record in records.iter().rev() {
        if seen.insert(record.turn_id.as_str()) {
            latest.push(record);
        }
    }
    latest.reverse();
    latest
}

fn push_bounded<'a>(target: &mut Vec<String>, values: impl Iterator<Item = &'a str>) {
    for value in values {
        let value = value.trim();
        if value.is_empty() || target.iter().any(|existing| existing == value) {
            continue;
        }
        if target.len() >= STATE_ITEM_LIMIT {
            return;
        }
        target.push(truncate_text(value, STATE_TEXT_LIMIT));
    }
}

fn keywords(document: &HistoryDocument) -> Vec<String> {
    let mut values = vec![markdown::document_title(&document.content, document.number)];
    values.extend(
        markdown::parse_initial_input_records(&document.content)
            .into_iter()
            .map(|record| record.raw_user_input),
    );
    values.extend(
        markdown::parse_checkpoint_records(&document.content)
            .into_iter()
            .flat_map(|record| [record.user_intent, record.raw_user_input]),
    );
    tokenize(&values.join(" ")).into_iter().take(32).collect()
}
