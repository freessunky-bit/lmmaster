//! Personas-Korea 데이터셋에서 조건 매칭 + 랜덤 샘플링.
//!
//! 정책:
//! - 데이터셋(.parquet)은 personas/ 폴더에 다운로드된 상태 가정 (v0.8.0).
//! - 모든 컬럼을 String으로 변환 후 HashMap에 보관 → 컬럼 스키마 변동에 견고.
//! - 필터: sex / age 범위 / region(province) substring / occupation substring / keyword(narrative).
//! - 모든 row를 메모리로 가져와 필터 + shuffle + truncate. 대용량은 batch 단위로 stream.

use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

use arrow_array::{
    Array, Float64Array, Int32Array, Int64Array, RecordBatch, StringArray, UInt32Array, UInt64Array,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PersonasSampleError {
    #[error("내부 오류: {message}")]
    Internal { message: String },
    #[error("데이터셋이 준비되지 않았어요. 1단계에서 데이터셋을 먼저 받아 주세요.")]
    NotReady,
    #[error("Parquet 파싱 실패: {message}")]
    ParquetParse { message: String },
}

/// 페르소나 추출 조건 — frontend가 자연어/폼/문서 파싱으로 만들어 전달.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PersonaFilter {
    /// "F" / "M" — None이면 전체.
    #[serde(default)]
    pub sex: Option<String>,
    #[serde(default)]
    pub age_min: Option<u32>,
    #[serde(default)]
    pub age_max: Option<u32>,
    /// 광역시도 부분 일치 OR. 빈 vec이면 전체.
    #[serde(default)]
    pub province_includes: Vec<String>,
    /// occupation 부분 일치 OR.
    #[serde(default)]
    pub occupation_includes: Vec<String>,
    /// persona narrative 부분 일치 OR (관심사·키워드).
    #[serde(default)]
    pub keyword_includes: Vec<String>,
    /// 추출 인원 수.
    pub sample_size: usize,
    /// 재현성용 시드. 없으면 OS RNG.
    #[serde(default)]
    pub seed: Option<u64>,
}

/// 추출된 페르소나 1명 — 모든 컬럼을 String으로.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub uuid: String,
    /// 자주 쓰는 핵심 필드 — UI 미리보기용.
    pub sex: String,
    pub age: String,
    pub province: String,
    pub occupation: String,
    /// LLM에 system prompt로 통째로 주입할 narrative.
    pub persona: String,
    /// 그 외 모든 컬럼 (educational_attainment 등).
    pub fields: HashMap<String, String>,
}

fn personas_dir(app: &AppHandle) -> Result<PathBuf, PersonasSampleError> {
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(|e| PersonasSampleError::Internal {
            message: format!("app_local_data_dir 실패: {e}"),
        })?
        .join("personas"))
}

fn list_parquet_files(dir: &std::path::Path) -> Result<Vec<PathBuf>, PersonasSampleError> {
    if !dir.exists() {
        return Err(PersonasSampleError::NotReady);
    }
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| PersonasSampleError::Internal {
        message: format!("read_dir: {e}"),
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("parquet"))
                .unwrap_or(false)
        {
            out.push(path);
        }
    }
    if out.is_empty() {
        return Err(PersonasSampleError::NotReady);
    }
    Ok(out)
}

/// arrow Array → 행 i의 값을 String으로 변환. 지원 안 하는 타입은 빈 문자열.
fn cell_to_string(array: &dyn Array, i: usize) -> String {
    if array.is_null(i) {
        return String::new();
    }
    if let Some(a) = array.as_any().downcast_ref::<StringArray>() {
        return a.value(i).to_string();
    }
    if let Some(a) = array.as_any().downcast_ref::<Int32Array>() {
        return a.value(i).to_string();
    }
    if let Some(a) = array.as_any().downcast_ref::<Int64Array>() {
        return a.value(i).to_string();
    }
    if let Some(a) = array.as_any().downcast_ref::<UInt32Array>() {
        return a.value(i).to_string();
    }
    if let Some(a) = array.as_any().downcast_ref::<UInt64Array>() {
        return a.value(i).to_string();
    }
    if let Some(a) = array.as_any().downcast_ref::<Float64Array>() {
        return a.value(i).to_string();
    }
    String::new()
}

/// RecordBatch → Vec<Persona> + 조건 필터링.
fn batch_to_personas(batch: &RecordBatch, filter: &PersonaFilter) -> Vec<Persona> {
    let schema = batch.schema();
    let cols: Vec<(String, &dyn Array)> = schema
        .fields()
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name().to_string(), batch.column(i).as_ref()))
        .collect();

    let mut out = Vec::new();
    let n = batch.num_rows();
    for i in 0..n {
        let mut fields: HashMap<String, String> = HashMap::with_capacity(cols.len());
        for (name, array) in &cols {
            fields.insert(name.clone(), cell_to_string(*array, i));
        }
        let uuid = fields.get("uuid").cloned().unwrap_or_default();
        let sex = fields.get("sex").cloned().unwrap_or_default();
        let age = fields.get("age").cloned().unwrap_or_default();
        let province = fields.get("province").cloned().unwrap_or_default();
        let occupation = fields
            .get("occupation")
            .cloned()
            .or_else(|| fields.get("occupation_modifier").cloned())
            .unwrap_or_default();
        // 우선순위: persona → professional_persona → 첫 narrative 컬럼.
        let persona = fields
            .get("persona")
            .cloned()
            .or_else(|| fields.get("professional_persona").cloned())
            .or_else(|| fields.get("sports_persona").cloned())
            .or_else(|| fields.get("hobbies_and_interests_persona").cloned())
            .unwrap_or_default();

        if !apply_filter(filter, &sex, &age, &province, &occupation, &persona, &fields) {
            continue;
        }

        out.push(Persona {
            uuid,
            sex,
            age,
            province,
            occupation,
            persona,
            fields,
        });
    }
    out
}

fn apply_filter(
    f: &PersonaFilter,
    sex: &str,
    age: &str,
    province: &str,
    occupation: &str,
    persona: &str,
    all: &HashMap<String, String>,
) -> bool {
    if let Some(want) = f.sex.as_deref() {
        if !want.is_empty() && sex.eq_ignore_ascii_case(want).not() {
            return false;
        }
    }
    if f.age_min.is_some() || f.age_max.is_some() {
        let age_n: u32 = age.parse().unwrap_or(0);
        if let Some(min) = f.age_min {
            if age_n < min {
                return false;
            }
        }
        if let Some(max) = f.age_max {
            if age_n > max {
                return false;
            }
        }
    }
    if !f.province_includes.is_empty()
        && !f
            .province_includes
            .iter()
            .any(|p| !p.is_empty() && province.contains(p.as_str()))
    {
        return false;
    }
    if !f.occupation_includes.is_empty()
        && !f
            .occupation_includes
            .iter()
            .any(|o| !o.is_empty() && occupation.contains(o.as_str()))
    {
        return false;
    }
    if !f.keyword_includes.is_empty() {
        // narrative 또는 모든 컬럼에서 substring 매치.
        let combined: String = std::iter::once(persona.to_string())
            .chain(all.values().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        if !f
            .keyword_includes
            .iter()
            .any(|k| !k.is_empty() && combined.contains(k.as_str()))
        {
            return false;
        }
    }
    true
}

trait BoolNot {
    fn not(self) -> Self;
}
impl BoolNot for bool {
    fn not(self) -> Self {
        !self
    }
}

/// 페르소나 샘플링 IPC. 모든 .parquet 순회 → 필터 → shuffle → truncate.
#[tauri::command]
pub async fn personas_sample(
    app: AppHandle,
    filter: PersonaFilter,
) -> Result<Vec<Persona>, PersonasSampleError> {
    let dir = personas_dir(&app)?;
    let files = list_parquet_files(&dir)?;

    let result =
        tokio::task::spawn_blocking(move || -> Result<Vec<Persona>, PersonasSampleError> {
            let mut all: Vec<Persona> = Vec::new();
            // 메모리 보호 — 누적 1M 페르소나 도달 시 break (필터 후 기준).
            const MEMORY_CAP: usize = 1_000_000;
            for path in files {
                let file = File::open(&path).map_err(|e| PersonasSampleError::Internal {
                    message: format!("file open {}: {e}", path.display()),
                })?;
                let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|e| {
                    PersonasSampleError::ParquetParse {
                        message: format!("{}: {e}", path.display()),
                    }
                })?;
                let reader =
                    builder
                        .build()
                        .map_err(|e| PersonasSampleError::ParquetParse {
                            message: format!("{}: {e}", path.display()),
                        })?;
                for batch_result in reader {
                    let batch = batch_result.map_err(|e| PersonasSampleError::ParquetParse {
                        message: format!("batch: {e}"),
                    })?;
                    let mut filtered = batch_to_personas(&batch, &filter);
                    all.append(&mut filtered);
                    if all.len() >= MEMORY_CAP {
                        break;
                    }
                }
                if all.len() >= MEMORY_CAP {
                    break;
                }
            }
            // shuffle + truncate.
            let seed = filter.seed.unwrap_or_else(|| {
                use std::time::{SystemTime, UNIX_EPOCH};
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(42)
            });
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            all.shuffle(&mut rng);
            let take = filter.sample_size.min(all.len());
            all.truncate(take);
            Ok(all)
        })
        .await
        .map_err(|e| PersonasSampleError::Internal {
            message: format!("blocking task: {e}"),
        })??;

    Ok(result)
}
