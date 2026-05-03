//! 디렉터리 기반 manifest 로더.
//!
//! 정책 (Phase 2'.a):
//! - `load_from_dir(path)` — 지정 디렉터리(재귀)에서 `*.json` 파일을 모두 읽어 ModelManifest 머지.
//! - 동일 id가 여러 파일에 있으면 **첫 번째**만 채택 — `paths.sort()`로 결정성 보장.
//! - JSON parse 실패는 즉시 에러 (cache poisoning 방지). 빈 디렉터리는 빈 리스트 반환.
//! - `load_layered(snapshot, overlay)` — overlay가 같은 id를 갖고 있으면 덮어쓰기, 새 id면 추가.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::manifest::{ModelEntry, ModelManifest};

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json parse failed at {path}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("schema_version unsupported: {0}")]
    SchemaUnsupported(u32),
}

const SUPPORTED_SCHEMA: u32 = 1;

/// 디렉터리에서 `*.json` 파일을 모두 읽어 ModelEntry로 평탄화.
///
/// 결과는 입력 파일의 `paths.sort()` 순서를 따른다 → 결정성 보장.
pub fn load_from_dir(dir: &Path) -> Result<Vec<ModelEntry>, CacheError> {
    let mut paths = collect_json_files(dir)?;
    paths.sort();

    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut entries: Vec<ModelEntry> = Vec::new();

    for p in paths {
        let body = fs::read_to_string(&p)?;
        let manifest: ModelManifest =
            serde_json::from_str(&body).map_err(|e| CacheError::Json {
                path: p.display().to_string(),
                source: e,
            })?;
        if manifest.schema_version != SUPPORTED_SCHEMA {
            return Err(CacheError::SchemaUnsupported(manifest.schema_version));
        }
        for entry in manifest.entries {
            if seen.contains_key(&entry.id) {
                tracing::warn!(
                    id = %entry.id,
                    path = %p.display(),
                    "duplicate model id, keeping first"
                );
                continue;
            }
            seen.insert(entry.id.clone(), entries.len());
            entries.push(entry);
        }
    }

    Ok(entries)
}

/// snapshot(번들) + overlay(사용자/원격 cache) 합치기.
///
/// 같은 id면 overlay가 덮어쓰고, 새 id면 추가. 순서는 snapshot → overlay.
pub fn load_layered(snapshot: Vec<ModelEntry>, overlay: Vec<ModelEntry>) -> Vec<ModelEntry> {
    let mut by_id: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<ModelEntry> = Vec::with_capacity(snapshot.len() + overlay.len());

    for e in snapshot {
        by_id.insert(e.id.clone(), out.len());
        out.push(e);
    }
    for e in overlay {
        if let Some(&idx) = by_id.get(&e.id) {
            out[idx] = e;
        } else {
            by_id.insert(e.id.clone(), out.len());
            out.push(e);
        }
    }
    out
}

fn collect_json_files(dir: &Path) -> Result<Vec<PathBuf>, CacheError> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            out.extend(collect_json_files(&path)?);
        } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
            out.push(path);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Maturity, ModelSource, VerificationInfo};
    use shared_types::{ModelCategory, RuntimeKind};
    use std::io::Write;

    fn write_manifest(path: &Path, entries: Vec<ModelEntry>) {
        let m = ModelManifest {
            schema_version: 1,
            generated_at: "2026-04-27T00:00:00Z".into(),
            entries,
        };
        let body = serde_json::to_string_pretty(&m).unwrap();
        let mut f = fs::File::create(path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    fn make_entry(id: &str) -> ModelEntry {
        ModelEntry {
            id: id.into(),
            display_name: id.into(),
            category: ModelCategory::AgentGeneral,
            model_family: "x".into(),
            source: ModelSource::DirectUrl {
                url: "https://x".into(),
            },
            runner_compatibility: vec![RuntimeKind::LlamaCpp],
            quantization_options: vec![],
            min_vram_mb: None,
            rec_vram_mb: None,
            min_ram_mb: 1024,
            rec_ram_mb: 2048,
            install_size_mb: 100,
            context_guidance: None,
            language_strength: Some(5),
            roleplay_strength: None,
            coding_strength: None,
            tool_support: false,
            vision_support: false,
            structured_output_support: false,
            license: "MIT".into(),
            maturity: Maturity::Stable,
            portable_suitability: 5,
            on_device_suitability: 5,
            fine_tune_suitability: 5,
            verification: VerificationInfo::default(),
            hf_meta: None,
            use_case_examples: vec![],
            notes: None,
            warnings: vec![],
            hub_id: None,
            tier: crate::manifest::ModelTier::default(),
            community_insights: None,
            intents: vec![],
            domain_scores: Default::default(),
            purpose: Default::default(),
            commercial: true,
            content_warning: None,
            mmproj: None,
        }
    }

    #[test]
    fn load_empty_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let entries = load_from_dir(tmp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn load_nonexistent_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does_not_exist");
        let entries = load_from_dir(&missing).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn load_single_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        write_manifest(&tmp.path().join("a.json"), vec![make_entry("alpha")]);
        let entries = load_from_dir(tmp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "alpha");
    }

    #[test]
    fn load_recursive_subdirs() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("agents");
        fs::create_dir(&sub).unwrap();
        write_manifest(&sub.join("a.json"), vec![make_entry("alpha")]);
        write_manifest(&tmp.path().join("b.json"), vec![make_entry("beta")]);
        let entries = load_from_dir(tmp.path()).unwrap();
        assert_eq!(entries.len(), 2);
        // paths.sort() — 결정성: agents/a.json이 b.json보다 먼저.
        assert_eq!(entries[0].id, "alpha");
        assert_eq!(entries[1].id, "beta");
    }

    #[test]
    fn duplicate_id_keeps_first() {
        let tmp = tempfile::tempdir().unwrap();
        write_manifest(&tmp.path().join("a.json"), vec![make_entry("dup")]);
        write_manifest(&tmp.path().join("b.json"), vec![make_entry("dup")]);
        let entries = load_from_dir(tmp.path()).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn invalid_json_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = tmp.path().join("bad.json");
        fs::write(&bad, "{not json").unwrap();
        let r = load_from_dir(tmp.path());
        assert!(matches!(r, Err(CacheError::Json { .. })));
    }

    #[test]
    fn unsupported_schema_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let body = r#"{ "schema_version": 99, "generated_at": "2026", "entries": [] }"#;
        fs::write(tmp.path().join("a.json"), body).unwrap();
        let r = load_from_dir(tmp.path());
        assert!(matches!(r, Err(CacheError::SchemaUnsupported(99))));
    }

    #[test]
    fn load_layered_overlay_overwrites_same_id() {
        let mut base = make_entry("dup");
        base.display_name = "base".into();
        let mut over = make_entry("dup");
        over.display_name = "override".into();
        let merged = load_layered(vec![base], vec![over]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].display_name, "override");
    }

    #[test]
    fn load_layered_overlay_appends_new_id() {
        let merged = load_layered(vec![make_entry("a")], vec![make_entry("b")]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].id, "a");
        assert_eq!(merged[1].id, "b");
    }

    #[test]
    fn load_layered_preserves_order() {
        let merged = load_layered(
            vec![make_entry("a"), make_entry("b"), make_entry("c")],
            vec![],
        );
        assert_eq!(
            merged.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }
}
