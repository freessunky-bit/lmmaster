# 가상 한국인 설문 시뮬레이션 — Nemotron + Personas-Korea + Workbench

> **무엇을 만드나요?** 엔비디아가 공개한 *Personas-Korea* 데이터셋(700만 합성 한국인 페르소나)에서 100명을 샘플링해, 로컬 LLM(Nemotron 3 Nano 4B 또는 EXAONE 7.8B)으로 콘텐츠 설문 응답을 시뮬레이션해요.
>
> **소요 시간**: 모델 다운로드 30~60분 + 데이터셋 5분 + 첫 배치 실행 5~10분.
>
> **얻는 것**: 100명의 가상 한국인이 같은 설문에 어떻게 답할지, 인구통계 분포 그대로의 응답 셋. 콘텐츠 A/B 테스트 사전 검증, 페르소나 마케팅 카피 검증, UX 리서치 가설 수립에 써요.

---

## 1. 준비물

| 항목 | 사양 |
|---|---|
| **PC RAM** | 최소 8GB (Nemotron 4B 기준), 권장 16GB |
| **VRAM** | 없어도 OK (CPU 추론 가능). 6GB+ 있으면 빠름 |
| **디스크** | 5GB (모델) + 2GB (데이터셋) + 1GB (결과) |
| **Python** | 3.10+ (데이터셋 샘플링 스크립트용) |
| **LMmaster** | v0.0.1 이상 |

> 한국어 *자연스러움*이 매우 중요하면 **EXAONE 3.5 7.8B Instruct**(8GB VRAM 권장)를 쓰는 게 좋아요. Nemotron 4B는 다국어 모델이라 한국어 단독 자연스러움은 한 단계 낮지만, **4B 사이즈 + 262K context + 빠른 배치 처리**라는 장점이 있어요. 100인 배치는 Nemotron이, 정성 분석 5~10인은 EXAONE이 권장이에요.

---

## 2. 단계별 셋업

### Step 1 — 모델 설치 (LMmaster 카탈로그)

LMmaster 실행 → **모델 카탈로그** → 🔥 NEW 탭.

- **NVIDIA Nemotron 3 Nano 4B (다국어, 한국어 포함)** 카드 클릭.
- 사양 확인 ("내 PC와 잘 맞아요" hint 체크) → **설치할게요** 버튼.
- 다운로드 진행 — 약 2.9GB (Q4_K_M 기준).
- 설치 완료 후 **채팅** 탭에서 빠른 테스트 ("안녕하세요. 자기소개해 주세요." 입력).

> ⚠️ Nemotron은 Mamba-2 + Transformer 하이브리드 아키텍처라 LMmaster 내장 llama.cpp가 처음 로드 시 약간 시간이 걸려요. 첫 응답에서 글자가 깨지면 LMmaster를 재시작 후 다시 시도해 주세요.

대안: **EXAONE 3.5 7.8B Instruct** (한국어 자연스러움 우선 시).

### Step 2 — Personas-Korea 데이터셋 다운로드

LMmaster 외부에서 진행 — Python으로:

```bash
pip install datasets pyarrow pandas
```

```python
# download_personas.py
from datasets import load_dataset
import pandas as pd

ds = load_dataset("nvidia/Nemotron-Personas-Korea", split="train")
df = ds.to_pandas()

# 결과: 1,000,000 rows × 26 columns
print(f"총 {len(df):,}명, 컬럼: {len(df.columns)}개")
df.to_parquet("personas_korea.parquet", compression="snappy")
```

> Parquet 포맷이라 5분 내외 완료, 1.8GB 차지해요. CSV 변환은 비추 (8GB+로 부풀어요).

### Step 3 — 100인 샘플링 (인구통계 비례)

단순 랜덤은 분포가 흔들려요. **stratified sampling** (성별/연령대/지역 비례)으로 100명 뽑아요.

```python
# sample_100.py
import pandas as pd
import numpy as np

df = pd.read_parquet("personas_korea.parquet")

# 연령대 그룹 + 성별 + 광역시도 3축 stratified
df["age_band"] = pd.cut(df["age"], bins=[18, 29, 39, 49, 59, 69, 99],
                        labels=["20대", "30대", "40대", "50대", "60대", "70대+"])

# 비례 표본 100명 — uuid hash로 결정성 보장
rng = np.random.RandomState(42)
sampled = (
    df.groupby(["age_band", "sex", "province"], observed=True, group_keys=False)
      .apply(lambda g: g.sample(min(len(g), max(1, int(round(len(g) / len(df) * 100)))),
                                random_state=rng))
)

# 정확히 100명으로 맞추기 (반올림 오차 보정)
if len(sampled) > 100:
    sampled = sampled.sample(100, random_state=rng)
elif len(sampled) < 100:
    extra = df.drop(sampled.index).sample(100 - len(sampled), random_state=rng)
    sampled = pd.concat([sampled, extra])

sampled = sampled.reset_index(drop=True)
sampled.to_json("personas_100.jsonl", orient="records", lines=True, force_ascii=False)
print(f"선정 완료: {len(sampled)}명")
```

`personas_100.jsonl`이 결과 파일. 한 줄당 한 명 — uuid, sex, age, province, occupation, persona narrative 등.

### Step 4 — 설문지 정의

설문은 자연어 질문 + 보기 형식. JSON 1개로 관리해요.

```json
{
  "survey_id": "content-pilot-2026-05",
  "title": "콘텐츠 선호도 사전 조사",
  "questions": [
    {
      "id": "q1",
      "type": "single",
      "text": "주말 저녁 60분이 비었어요. 다음 중 어떤 콘텐츠를 가장 보고 싶으세요?",
      "options": ["드라마 1편", "예능 1편", "영화 1편", "유튜브 짧은 영상 여러 개", "관심 없음 / 다른 활동"]
    },
    {
      "id": "q2",
      "type": "scale",
      "text": "최근 한 달 동안 OTT(넷플릭스/티빙/쿠팡플레이 등) 사용 빈도는?",
      "scale": "1=전혀 안 씀, 5=거의 매일"
    },
    {
      "id": "q3",
      "type": "open",
      "text": "최근 가장 인상 깊게 본 한국 콘텐츠 한 작품과 그 이유를 한두 문장으로 적어 주세요."
    }
  ]
}
```

`survey.json`으로 저장.

### Step 5 — Workbench batch 실행 (LMmaster)

LMmaster의 **워크벤치** 탭으로 이동.

#### 5-1. System prompt 템플릿

페르소나 narrative를 system prompt로 주입해 모델이 그 사람의 입장에서 답하도록 해요.

```
당신은 다음 인구통계와 배경을 가진 한국인입니다. 모든 답변은 이 사람의 입장에서, 한국어 해요체 또는 평소 말투로, 자연스럽게 작성해 주세요.

이름: (uuid 생략, 익명)
성별: {sex}
나이: {age}세
거주지: {province} {district}
직업: {occupation}
교육: {education_level}
가족: {family_type}

배경 페르소나:
{persona}

문화적 배경:
{cultural_background}

취미/관심사:
{hobbies_and_interests}

설문 응답 시 규칙:
1. 객관식은 보기 중 하나만 정확히 골라 답합니다.
2. 척도(1-5)는 숫자만 답합니다.
3. 주관식은 1-2문장 자연스러운 한국어로 답합니다.
4. 본인의 인구통계/배경과 어울리지 않는 답은 피합니다.
```

#### 5-2. User prompt 템플릿 (질문별)

```
[설문 q{id}] {text}

{options 보기 1줄씩 또는 척도 설명}

답변:
```

#### 5-3. 100인 × 3문항 = 300회 호출 — Python 스크립트로 LMmaster Local API 호출

LMmaster는 OpenAI 호환 로컬 API를 제공해요 (기본 포트는 LMmaster 홈 화면에서 확인).

```python
# run_survey.py
import json, time
from pathlib import Path
import requests

BASE = "http://127.0.0.1:8788/v1"  # LMmaster 홈 화면의 게이트웨이 포트로 교체
MODEL = "nemotron-3-nano-4b"        # 또는 "exaone-3.5-7.8b-instruct"

personas = [json.loads(l) for l in Path("personas_100.jsonl").read_text(encoding="utf-8").splitlines()]
survey = json.loads(Path("survey.json").read_text(encoding="utf-8"))

def system_prompt(p):
    return f"""당신은 다음 인구통계와 배경을 가진 한국인입니다...
성별: {p['sex']}
나이: {p['age']}세
거주지: {p['province']} {p.get('district', '')}
직업: {p['occupation']}
교육: {p['education_level']}
가족: {p['family_type']}

배경 페르소나:
{p['persona']}

문화적 배경:
{p['cultural_background']}

취미/관심사:
{p['hobbies_and_interests']}

규칙: 객관식은 보기 1개만, 척도는 숫자만, 주관식은 1-2문장."""

def user_prompt(q):
    if q["type"] == "single":
        opts = "\n".join(f"- {o}" for o in q["options"])
        return f"[{q['id']}] {q['text']}\n\n{opts}\n\n답변:"
    elif q["type"] == "scale":
        return f"[{q['id']}] {q['text']}\n({q['scale']})\n\n답변(숫자만):"
    else:
        return f"[{q['id']}] {q['text']}\n\n답변:"

results = []
for i, p in enumerate(personas):
    for q in survey["questions"]:
        resp = requests.post(f"{BASE}/chat/completions", json={
            "model": MODEL,
            "messages": [
                {"role": "system", "content": system_prompt(p)},
                {"role": "user", "content": user_prompt(q)},
            ],
            "temperature": 0.7,
            "max_tokens": 256,
        }, timeout=120).json()
        ans = resp["choices"][0]["message"]["content"].strip()
        results.append({
            "persona_uuid": p["uuid"],
            "sex": p["sex"], "age": p["age"], "province": p["province"],
            "question_id": q["id"], "answer": ans,
        })
    if (i + 1) % 10 == 0:
        print(f"진행: {i+1}/100명")

Path("survey_results.jsonl").write_text(
    "\n".join(json.dumps(r, ensure_ascii=False) for r in results),
    encoding="utf-8",
)
print("완료 — survey_results.jsonl")
```

> 100인 × 3문항 = 300회 호출. Nemotron 4B + RTX 3060 기준 약 5~7분, CPU만이면 30~45분이에요. 진행 동안 LMmaster의 진단 탭에서 GPU/메모리 상태 모니터링 가능.

### Step 6 — 결과 집계

```python
# aggregate.py
import pandas as pd

df = pd.read_json("survey_results.jsonl", lines=True)

# Q1 객관식 — 인구통계 cross-tab
q1 = df[df["question_id"] == "q1"].copy()
q1["answer_norm"] = q1["answer"].str.split("\n").str[0].str.strip()  # 첫 줄만
print("=== Q1: 주말 저녁 콘텐츠 선호 ===")
print(pd.crosstab(q1["age"].pipe(lambda s: pd.cut(s, [18, 29, 39, 49, 99], labels=["20대", "30대", "40대", "50+"])),
                  q1["answer_norm"], normalize="index").round(2))

# Q2 척도 — 평균 + 분포
q2 = df[df["question_id"] == "q2"].copy()
q2["score"] = q2["answer"].str.extract(r"(\d)").astype(float)
print(f"\n=== Q2: OTT 빈도 평균 {q2['score'].mean():.2f} (n={q2['score'].notna().sum()}) ===")

# Q3 주관식 — 키워드 빈도
from collections import Counter
words = Counter()
for ans in df[df["question_id"] == "q3"]["answer"]:
    for w in ans.split():
        if len(w) >= 2:
            words[w] += 1
print(f"\n=== Q3: 자주 등장한 단어 Top 20 ===")
for w, c in words.most_common(20):
    print(f"{c:3d}  {w}")
```

CSV/Excel로 export하려면 `df.to_excel("survey.xlsx")`.

---

## 3. 결과 해석 시 주의사항

- **합성 페르소나 ≠ 실제 사용자**: Personas-Korea는 KOSIS/대법원/건강보험공단 *통계 분포*를 모방한 합성이라 실제 한국인 *개인*의 의견과는 달라요. 마케팅 가설 수립이나 카피 사전 검증에는 유용하지만, 실제 사용자 인터뷰를 *대체*하지는 않아요.
- **모델 편향 주의**: 모델이 학습한 데이터(서구권 비중 큰 일반 corpus)가 한국 문화 맥락을 완벽히 반영 못 할 수 있어요. 예: 50대 농촌 여성 응답이 20대 도시 여성과 비슷하게 나오면 모델이 페르소나 narrative를 무시한 신호 — system prompt를 더 구체적으로 주거나, **EXAONE 7.8B로 교체** 해 보세요.
- **temperature**: 0.7이 다양성/일관성 균형. 0.3~0.5는 보수적, 0.9+는 흩뿌려져요.
- **재현성**: `random_state=42` 고정 + temperature=0 / seed 고정으로 같은 결과 재현 가능. 단 Nemotron Mamba 부분은 batch 크기에 따라 미세 차이.

---

## 4. 라이선스 및 출처 표기

| 자산 | 라이선스 | 사용 시 표기 |
|---|---|---|
| **Personas-Korea 데이터셋** | CC BY 4.0 | "데이터: NVIDIA Nemotron-Personas-Korea (KOSIS·대법원·건강보험공단 통계 기반 합성)" |
| **Nemotron 3 Nano 4B** | NVIDIA Open Model License | "추론: NVIDIA Nemotron 3 Nano 4B" — 상업 사용 OK |
| **EXAONE 3.5 7.8B** | EXAONE Custom License | LG AI Research 약관 검토 후 사용. 일부 상업 제약 |

연구 발표 / 보고서 작성 시 **NVIDIA + 통계 출처 4곳 모두 명시**가 CC BY 4.0의 요구사항이에요.

---

## 5. 자주 만나는 문제

| 증상 | 원인 / 해결 |
|---|---|
| 한국어 답변에 영어 단어 섞임 | Nemotron은 다국어 학습이라 종종 발생. system prompt에 "*반드시 한국어로만*" 강조 추가, 또는 EXAONE 교체. |
| 모든 페르소나가 비슷한 답 | system prompt에 페르소나 narrative가 충분히 안 들어감. `persona` + `cultural_background` 둘 다 포함했는지 확인. temperature 0.8~0.9로 상향. |
| 객관식이 보기 외 답변 | "보기 중 정확히 하나만"을 system prompt 마지막 줄에 강조. user prompt 끝에 "답변(보기 중 하나):" 추가. |
| 4B 모델인데 메모리 부족 | 다른 앱 종료 후 재시도. 또는 LMmaster 설정에서 KV cache 크기 축소. |
| 응답이 너무 짧음 | `max_tokens` 256→512로 상향. 주관식만 별도로 token 늘려도 OK. |

---

## 6. 다음 단계

- **확장 — 1000인 시뮬레이션**: 위 스크립트의 `100`을 `1000`으로. RTX 3060 기준 1시간 정도. Stratified 샘플링이 1000인에서 더 정확한 분포 재현해요.
- **A/B 카피 테스트**: 같은 페르소나 100인에 두 가지 광고 카피를 보여주고 선호도 비교 — Q4/Q5에 카피 보기 추가.
- **Workbench LoRA 미세조정**: Personas-Korea를 fine-tune corpus로 써서 *한국어 페르소나 임베딩이 더 강한* 모델 만들기 (Workbench 사다리 3단계).
- **RAG 시드로 활용**: 워크스페이스 RAG에 페르소나 narrative 임베딩 → 챗봇이 질의 시 비슷한 페르소나를 참조해 답변 생성.

---

## 출처

- [nvidia/Nemotron-Personas-Korea](https://huggingface.co/datasets/nvidia/Nemotron-Personas-Korea) — 데이터셋
- [nvidia/NVIDIA-Nemotron-3-Nano-4B-GGUF](https://huggingface.co/nvidia/NVIDIA-Nemotron-3-Nano-4B-GGUF) — 모델
- [How to Ground a Korean AI Agent in Real Demographics with Synthetic Personas](https://huggingface.co/blog/nvidia/build-korean-agents-with-nemotron-personas) — NVIDIA 블로그
- [LMmaster RESUME.md](../RESUME.md), [PRODUCT.md](../PRODUCT.md) — 프로젝트 컨텍스트
