<!-- section: getting-started -->
# 시작하기

LMmaster를 처음 여시면 사용자 동의서(EULA)와 4단계 마법사가 안내를 시작해요. 마법사가 끝나면 곧바로 메인 화면으로 들어와요.

## 첫 실행 흐름

- **1단계 — 언어 선택**: 한국어와 English 중 골라요. 이후 Settings에서 바꿀 수 있어요.
- **2단계 — PC 점검**: GPU, RAM, OS를 살펴서 어떤 모델이 잘 맞을지 미리 알려드려요.
- **3단계 — 런타임 설치**: Ollama는 자동으로 받고, LM Studio는 공식 사이트로 안내해드려요.
- **4단계 — 준비 완료**: 추천 모델 한 개를 바로 받을 수도 있고, 카탈로그에서 천천히 골라도 돼요.

## 메인 화면 구성

- 왼쪽 사이드바에 모든 기능 메뉴가 있어요.
- 위쪽 표시줄에 현재 화면 이름과 게이트웨이 상태가 보여요.
- 게이트웨이 포트(예: 11434)는 외부 웹앱이 LMmaster를 부를 때 쓰는 주소예요.

## 단축키

- **Ctrl+K** (Windows) 또는 **⌘K** (mac) — 명령 팔레트.
- **F1** 또는 **Shift+?** — 단축키 도움말.
- **Ctrl+1~9** — 메뉴 빠른 이동.

---

<!-- section: catalog -->
# 모델 카탈로그

추천 strip이 PC에 잘 맞는 모델 3종을 가장 위에 보여드려요. 그 아래 카테고리 탭으로 더 많은 모델을 살펴볼 수 있어요.

## 추천 받는 흐름

- 카탈로그에 들어가면 PC를 30초 동안 측정해서 점수를 매겨요.
- "**상위 품질** · **균형형** · **경량형**" 세 종류로 추천이 갈려요.
- 추천 카드에는 한국어 이유 한 줄이 함께 있어요. 예: "이 PC와 잘 맞아요".

## 카테고리 살펴보기

- 카테고리 탭에서 **요약**, **번역**, **코드**, **대화** 등 용도별로 좁힐 수 있어요.
- 정렬은 **추천순**, **VRAM 적은 순**, **이름순**으로 바꿀 수 있어요.

## 모델 설치하기

- 카드를 누르면 자세한 정보(라이선스, 사이즈, 양자화 옵션)가 떠요.
- "**이 모델 설치할게요**" 버튼을 누르면 설치 센터로 가요.
- 설치 센터에서 진행률, ETA(남은 시간 안내), 속도를 볼 수 있어요.

## 사용자 정의 모델

- 워크벤치에서 만든 모델은 카탈로그 위쪽 "**내가 만든 모델**" 섹션에 자동으로 표시돼요.
- 일반 카드와 동일하게 클릭해서 자세히 보거나 등록 정보를 확인할 수 있어요.

---

<!-- section: chat -->
# 채팅으로 시험하기

받은 모델이 잘 작동하는지 LMmaster 안에서 바로 채팅으로 확인할 수 있어요. 외부 도구(Ollama 명령줄, LM Studio 데스크톱 앱)에서 같은 모델을 어떻게 부르는지도 함께 안내해요.

## LMmaster 안에서 채팅

- 사이드바 **채팅** 메뉴를 누르면 받아둔 Ollama 모델이 자동으로 드롭다운에 떠요.
- 메시지를 입력하고 **Enter**로 보내요. **Shift+Enter**는 줄바꿈이에요.
- 응답은 토큰 단위로 흘러나오고, 완료되면 응답 시간(예: `2.4초`)이 표시돼요.
- 여러 turn 대화는 자동으로 history가 같이 전달돼요. 처음부터 다시 하려면 **처음부터 다시** 버튼.
- 위로 스크롤해서 이전 메시지를 읽는 동안엔 자동 스크롤이 멈춰요. 새 응답이 쌓이면 우측 하단에 **맨 아래로 ↓** 버튼이 떠요.

## 외부 도구에서 같은 모델 쓰기

LMmaster가 받은 모델은 Ollama 데몬이 보관해요(`%USERPROFILE%\.ollama\models` / `~/.ollama/models`). 다른 도구에서도 그대로 호출할 수 있어요.

### 1) Ollama 명령줄

```bash
# 받은 모델 목록 보기
ollama list

# 바로 채팅 (선택한 모델 이름)
ollama run sam860/exaone-4.0:1.2b

# 단일 prompt — 스크립트/파이프라인용
ollama run sam860/exaone-4.0:1.2b "한국어로 자기소개해 주세요"

# OpenAI 호환 endpoint (외부 앱 연동)
curl http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"sam860/exaone-4.0:1.2b","messages":[{"role":"user","content":"안녕"}]}'
```

- LMmaster의 **로컬 API** 메뉴에서 발급한 키를 쓰면 LMmaster gateway 경유 호출도 가능해요. 그러면 Pipelines 검사도 같이 적용돼요.
- Ollama는 5분간 모델을 메모리에 유지해요. 빈번한 호출이면 첫 응답만 느리고 다음부터 빨라져요.

### 2) LM Studio 데스크톱 앱

LM Studio는 자체 GGUF 폴더를 써요(`~/.cache/lm-studio/models` 또는 사용자 지정). LMmaster가 받은 Ollama 모델을 LM Studio에서 그대로 보려면 GGUF 파일을 LM Studio 폴더로 복사해야 해요.

- **간단한 길**: LM Studio 앱 안에서 동일 모델을 직접 검색해서 받기 — Ollama와 LM Studio는 모델을 따로 보관해요.
- **고급**: Ollama blob을 LM Studio로 복사 (한 번만):
  ```bash
  # Ollama가 저장한 GGUF 위치
  ls ~/.ollama/models/blobs/
  
  # LM Studio가 인식하는 폴더 구조로 symlink 또는 복사
  mkdir -p "~/.cache/lm-studio/models/공유/exaone"
  ln -s ~/.ollama/models/blobs/sha256-<digest> ~/.cache/lm-studio/models/공유/exaone/exaone-4.0-1.2b.Q4_K_M.gguf
  ```
- LM Studio는 Modelfile/template을 자체 형식으로 변환해요. EXAONE 같은 특수 architecture는 LM Studio Hub에서 직접 받는 것이 chat template 호환성 측면에서 더 안전해요.

### 3) llama.cpp / koboldcpp

```bash
# llama.cpp portable
./llama-server -m ~/.ollama/models/blobs/sha256-<digest> -c 4096 -ngl 99

# 그러면 http://localhost:8080에서 OpenAI 호환 endpoint 노출
```

---

<!-- section: model-tuning -->
# 모델별 권장 세팅

같은 모델도 어떻게 부르느냐에 따라 응답 품질이 크게 달라져요. 카탈로그가 큐레이션한 모델별 권장 파라미터와 system prompt 패턴을 정리했어요.

## 공통 파라미터 가이드

| 파라미터 | 의미 | 권장 |
|---|---|---|
| **temperature** | 창의성 vs 일관성 | 사실 답변 0.2~0.5 / 일반 대화 0.7 / 창작 0.85~1.0 |
| **top_p** | 후보 토큰 풀 | 0.9~0.95 (대부분 모델) |
| **top_k** | 후보 개수 | 40 (대부분) — 매우 보수적이려면 20 |
| **repeat_penalty** | 반복 억제 | 1.05~1.15 (한국어는 조사 반복 잦아 1.1 권장) |
| **num_ctx** | context 길이 | RAM 여유에 맞춰 — 4096이 안전 / 한국어 긴 문서 8192 |

## 모델별 권장 (Ollama 호출 기준)

### EXAONE 4.0 1.2B Instruct (한국어 비서)

- system prompt: `당신은 정확하고 친절한 한국어 AI 비서예요. 모르는 것은 모른다고 솔직히 말해요.`
- temperature 0.7 / top_p 0.95 / repeat_penalty 1.1 / num_ctx 8192
- stop sequence: `[|endofturn|]` 자동 처리됨 (sam860 wrapper 사용 시)
- 강점: 한국어 일상 대화 / 짧은 글쓰기. 1.2B라 사실관계 길게 묻지 말기.

### EXAONE 3.5 7.8B Instruct (한국어 + 추론)

- system prompt: 위와 동일 또는 비워둠.
- temperature 0.6 / top_p 0.95 / repeat_penalty 1.1 / num_ctx 8192
- 강점: 한국어 추론, 요약. 코딩은 약함 — Qwen Coder 권장.

### Qwen 2.5 Coder 3B Instruct (코딩)

- system prompt: `You are an expert software engineer. Answer in 한국어 unless the user asks otherwise. Show code in fenced blocks.`
- temperature 0.2~0.4 / top_p 0.9 / repeat_penalty 1.05 / num_ctx 8192~16384
- 강점: TypeScript / Python / Rust. 한국어 주석 잘 받아요.

### Llama 3.2 3B Instruct (범용 경량)

- system prompt: `You are a helpful assistant. Respond in 한국어 when the user writes in 한국어.`
- temperature 0.7 / top_p 0.9 / repeat_penalty 1.1 / num_ctx 8192
- 강점: 빠른 응답. 한국어는 EXAONE/Qwen이 더 자연스러움.

### Polyglot-Ko 12.8B (롤플레이 / 한국어 base)

- **base model이라 instruct template이 없어요**. system prompt 대신 형식 예시를 주는 few-shot 방식 권장.
- temperature 0.85 / top_p 0.95 / repeat_penalty 1.15 / num_ctx 4096
- 사용 패턴:
  ```
  나: 안녕
  봇: 반가워요. 오늘은 어떤 이야기 해볼까요?
  나: {사용자 입력}
  봇:
  ```
- 강점: 한국어 톤 / 캐릭터 롤플레이. 사실관계는 약함.

### HyperCLOVA X SEED 8B (Naver)

- system prompt: 한국어 비서 패턴 그대로.
- temperature 0.6 / top_p 0.95 / repeat_penalty 1.1 / num_ctx 8192
- 강점: 한국 문화 / 시사. 라이선스 주의 (상용 시 Naver 약관 확인).

## 채팅에서 직접 적용하기

LMmaster 채팅 페이지는 v1에서 기본 파라미터로 호출해요. 더 정밀한 튜닝이 필요하면:

1. Ollama 명령줄에서 `/set parameter` 명령으로 즉석 변경:
   ```
   /set parameter temperature 0.4
   /set parameter num_ctx 16384
   ```
2. 또는 **Modelfile**을 직접 작성해 새 모델로 등록 (워크벤치 5단계 흐름).

## 응답이 이상할 때 점검

- **무한 반복** → repeat_penalty 1.1~1.2로 올려요.
- **너무 짧은 답변** → max_tokens 또는 num_predict 4096+ 명시.
- **영어로 답함** → system prompt에 `한국어로 답해 주세요` 추가.
- **이상한 토큰 (`[|user|]` 등) 노출** → chat template이 GGUF에 없는 경우. Ollama Hub의 wrapper 모델(예: `sam860/exaone-4.0`)로 교체.
- **너무 느림** → num_ctx 줄이기 (8192 → 4096), 더 작은 모델로 교체, 또는 GPU offload (`-ngl 99`) 사용.

---

<!-- section: workbench -->
# 워크벤치

워크벤치는 5단계로 모델을 직접 가공해서 등록하는 공간이에요. 데이터 → 양자화 → LoRA 미세조정 → 검증 → 레지스트리 등록 흐름이에요.

## 5단계 흐름

- **1) Data**: JSONL 파일을 미리 보면서 학습 데이터를 정리해요. 한국어 카드와 영어 카드 모두 OK예요.
- **2) Quantize**: Q4_K_M, Q5_K_M, Q8_0, FP16 중에서 골라요. 작을수록 빠르지만 품질도 같이 떨어져요.
- **3) LoRA**: 미세 조정 epoch 수와 한국어 강도 옵션을 정해요.
- **4) Validate**: 작은 평가 셋으로 결과를 점수화해요. 카테고리별 통계를 볼 수 있어요.
- **5) Register**: model-registry에 등록하면 카탈로그에 나타나요. Ollama Modelfile도 함께 만들어요.

## 응답기(런타임) 선택

- **mock**: 외부 통신 없이 빠르게 검증할 때 써요.
- **ollama**: 로컬 Ollama 서버에 연결해서 실측 점수를 받아요.
- **lm-studio**: LM Studio HTTP 서버를 호출해요.
- 응답기 변경 후에는 base URL도 같이 확인해 주세요.

## 작업 멈추기

- 진행 중에 "**그만둘게요**"를 누르면 안전하게 정리해요.
- 다음 실행 때 이어서 할 수 있는 자료(JSONL, LoRA 가중치)는 보존돼요.
- 아주 오래된 임시 파일은 자동 정리 정책으로 사라져요. 설정에서 직접 정리할 수도 있어요.

---

<!-- section: knowledge -->
# 자료 인덱싱 (RAG)

워크스페이스의 **지식 자료** 탭에서 문서를 모델에 가르쳐 둘 수 있어요. RAG는 검색해서 관련 청크를 모델 컨텍스트에 끼워주는 기법이에요.

## 자료 받기

- 절대 경로(예: `C:/Users/me/notes`)를 적고 "**파일 1개**" 또는 "**폴더 (재귀)**"를 골라요.
- "**인덱싱 시작**"을 누르면 읽기 → 분할 → 임베딩 → 쓰기 단계가 자동으로 진행돼요.
- 같은 워크스페이스에서는 한 번에 하나만 인덱싱할 수 있어요.

## 검색하기

- 검색어를 적고 결과 개수(1~20)를 정해요.
- 결과는 코사인 유사도가 높은 청크 순으로 나와요.
- 결과 카드에는 원본 파일 경로가 함께 보여서 출처 추적이 쉬워요.

## 워크스페이스 격리

- 각 워크스페이스는 자기 자료만 검색해요. 다른 워크스페이스의 자료가 섞이지 않아요.
- 사이드바에서 워크스페이스를 바꾸면 자료도 함께 전환돼요.
- 워크스페이스 단위로 백업·이전이 가능해요(아래 "포터블 이동" 참고).

---

<!-- section: api-keys -->
# API 키 + 외부 웹앱

LMmaster는 OpenAI 호환 게이트웨이를 켜둬요. 외부 웹앱이 base URL만 LMmaster의 로컬 주소로 바꾸면 그대로 쓸 수 있어요.

## 키 발급

- "**로컬 API**" 메뉴에서 "**키 만들게요**" 버튼을 눌러요.
- 발급 직후 한 번만 평문으로 보여요. 잊지 말고 안전한 곳에 적어 두세요.
- 한 번 닫으면 다시 볼 수 없어요. 새 키를 다시 발급해야 해요.

## 키 범위(scope)

- **허용 모델**: 이 키로 부를 수 있는 모델 목록.
- **허용 출처(origins)**: CORS 검사 대상 도메인.
- **만료 시간**: 자동으로 만료되도록 시간을 정할 수 있어요. 무기한도 가능해요.

## 외부 웹앱 연결

- 웹앱 설정에서 base URL을 `http://127.0.0.1:포트번호`로 바꿔요.
- 키는 `Authorization: Bearer sk-lm-...` 헤더에 넣어요.
- 모델 이름은 OpenAI 호환 형태(예: `gpt-4o-mini`)도 가능하고, LMmaster 등록 모델 ID로도 가능해요.

## 키 회수

- "프로젝트" 메뉴에서 누가 어떤 키를 언제 썼는지 확인할 수 있어요.
- 의심스러운 키는 곧바로 "**키 회수**"로 무효화할 수 있어요.

---

<!-- section: portable -->
# 포터블 이동

워크스페이스 전체를 단일 zip 파일로 내보내고, 다른 PC에서 그대로 가져올 수 있어요. ADR-0009 "포터블 워크스페이스" 약속의 사용자 경험이에요.

## 내보내기

- "**설정 → 포터블 이동 → 다른 PC로 옮길게요**"를 눌러요.
- 옵션을 정해요:
  - **모델 포함**: 끄면 메타데이터만 (수 MB), 켜면 모델 파일도 (수 GB).
  - **키 포함**: 끄면 안전, 켜면 패스프레이즈(암호)로 잠가요.
- 진행률, ETA, sha256 해시가 차례로 보여요.

## 가져오기

- "**이 PC로 가져올게요**"를 누르고 zip 파일을 골라요.
- 받은 zip을 미리보기 단계에서 확인할 수 있어요(언제, 어디서 만들었는지).
- 다른 OS 계열의 zip은 모델을 다시 받아야 해요. fingerprint repair tier가 자동으로 안내해요.

## 안전 정책

- export 중 손상이 감지되면 즉시 멈춰요. 임시 파일은 자동 정리돼요.
- 키 패스프레이즈는 LMmaster가 보관하지 않아요. 잃어버리면 키는 복구할 수 없어요.
- USB나 클라우드 동기화로 옮겨도 OK예요. 같은 OS·아키텍처 PC 간 호환이에요.

---

<!-- section: diagnostics -->
# 자가 점검 + 자동 갱신

진단 메뉴에서 PC 상태와 LMmaster 동작을 한눈에 확인할 수 있어요. Settings에서 자동 갱신 주기도 정할 수 있어요.

## 자가 점검

- 진단 화면 위쪽에 GPU, VRAM, RAM, 디스크 여유 공간이 표시돼요.
- 자가 점검은 처음 시작할 때 1회 + Settings에서 정한 주기마다 자동 실행돼요.
- 결과 요약은 한국어로 짧게 보여드려요. 원본 로그는 "**자세히 보기**"로 펼쳐요.

## 게이트웨이 진단

- 현재 포트, 응답 시간, 마지막 요청을 볼 수 있어요.
- 외부 웹앱이 잘 연결됐는지 확인할 때 유용해요.

## 자동 갱신

- 6시간마다 GitHub Releases를 한 번씩 확인해요(외부 통신은 이게 전부예요).
- 새 버전이 있으면 우측 하단 toast로 알려드려요.
- "이번 버전은 건너뛸게요"를 누르면 그 버전은 다시 알려드리지 않아요.
- Settings에서 자동 갱신 자체를 끌 수도 있어요.

---

<!-- section: faq -->
# 자주 묻는 질문 (FAQ)

## 모델이 너무 느려요

- VRAM이 부족하면 더 작은 양자화(Q4_K_M)를 써 보세요.
- CPU 모드로 돌아가는 모델일 수 있어요. 카탈로그 카드의 "VRAM 권장"을 확인해 주세요.

## 게이트웨이 포트가 늘 바뀌어요

- 다른 프로그램이 그 포트를 쓰고 있을 때 LMmaster가 자동으로 다음 포트를 골라요.
- Settings의 "고정 포트"에서 명시 포트를 정할 수 있어요(있는 경우).

## 검색 결과가 비어 있어요

- 워크스페이스가 비어 있을 수 있어요. "지식 자료"에서 폴더를 인덱싱해 주세요.
- 다른 워크스페이스에 자료가 있을 수 있어요. 사이드바에서 워크스페이스를 확인해 주세요.

## 포터블 이동이 실패해요

- 디스크 여유 공간을 확인해 주세요. 모델 포함 zip은 수 GB가 될 수 있어요.
- target 경로에 쓰기 권한이 있는지 확인해 주세요.

## 외부 웹앱에서 401이 나요

- API 키가 만료됐거나 회수됐을 수 있어요. "로컬 API" 메뉴에서 새 키를 만들어 보세요.
- "허용 출처(origins)"에 그 웹앱 도메인이 포함됐는지 확인해 주세요.

## 단축키 표

- **Ctrl+K** / **⌘K** — 명령 팔레트
- **F1** / **Shift+?** — 단축키 도움말
- **Ctrl+1~9** / **⌘1~9** — 메뉴 빠른 이동
- **Esc** — 모달, 드로어, 팔레트 닫기
