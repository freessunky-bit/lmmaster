# 클라우드 GPU로 LMmaster 모델 내보내기 (한국어)

> Phase 8'.c.4 / ADR-0066 부산물 — 5G/공공 인터넷 베타 테스트 시나리오에서 LMmaster의 *모델 결정*을 그대로 가져가서 *클라우드 GPU*에서 서빙하는 흐름을 안내합니다.
>
> LMmaster v1은 **localhost 기반 컴패니언 도구**라 클라우드에 직접 배포되지 않아요. 대신 *모델 가중치(GGUF) + 런타임(llama.cpp)* 을 분리해서 클라우드에 가져갈 수 있어요.

---

## 누가 이 가이드를 보면 좋아요?

- 회사 PC에서 LMmaster로 모델 큐레이션을 했지만, 외부 사용자(5G 접속) 베타 테스트가 필요한 분.
- 본인 자택 PC GPU가 약하지만 클라우드 GPU로 임시 서빙하고 싶은 분.
- LMmaster의 한국어 모델 추천 + 벤치 결과는 그대로 살리되, 실제 추론은 분리하고 싶은 분.

---

## 큰 그림

```
┌─ LMmaster 띄운 PC ──────────────────────────────┐
│  - 한국어 카탈로그에서 모델 결정                │
│  - 30초 벤치로 PC 적합성 확인                   │
│  - 양자화 결정 (Q4_K_M / Q5_K_M / Q8_0 등)      │
│  - 모델 ID 받아 적기 (예: qwen-3-30b-a3b)       │
└──────────────────────────────────────────────────┘
                  ↓ (모델 ID + 양자화 정보만 전달)
┌─ 클라우드 GPU 인스턴스 (RunPod / Vast.ai 등) ───┐
│  - HF Hub에서 동일 모델 직접 pull               │
│  - llama.cpp server 헤드리스 실행               │
│  - public HTTPS endpoint 노출                   │
└──────────────────────────────────────────────────┘
                  ↓
            5G 폰 / 외부 사용자
```

LMmaster는 **결정 도구**, 클라우드는 **서빙 인프라**. 역할 분리.

---

## 1. LMmaster에서 모델 결정 + 정보 추출

1. 카탈로그에서 적합한 모델 고르기 (한국어 강도 / VRAM 권장 / 라이선스).
2. 30초 벤치 돌려서 *내 PC에서* 어떻게 도는지 확인. 토큰/초·TTFT 메모.
3. 양자화 옵션 결정 (모델 카드 페이지의 "양자화" 섹션):
   - **Q4_K_M** — 가장 균형. 대부분 모델의 추천 default.
   - **Q5_K_M** — 약간 더 무겁지만 한국어 자연스러움 보존.
   - **Q8_0** — 거의 BF16 수준. VRAM 여유 있을 때.
4. 모델 ID + HF Hub 경로 받아 적기. 예시:
   - `qwen-3-30b-a3b` → `Qwen/Qwen3-30B-A3B-Instruct-GGUF` (HF repo)
   - `exaone-3-5-32b` → `LGAI-EXAONE/EXAONE-3.5-32B-Instruct-GGUF`
   - `gemma-3-4b` → `bartowski/google_gemma-3-4b-it-GGUF`

> **팁**: 카탈로그 카드의 "HuggingFace에서 보기" 링크를 클릭하면 정확한 repo URL을 얻을 수 있어요.

---

## 2. 모델 파일 위치 (이미 받은 모델 재활용 시)

LMmaster가 받은 GGUF 파일은 다음 위치에 있어요:

```
Windows: %LOCALAPPDATA%\com.mojito.lmmaster\models\
macOS:   ~/Library/Application Support/com.mojito.lmmaster/models/
Linux:   ~/.local/share/com.mojito.lmmaster/models/
```

Settings → 고급 → "모델 폴더 열기" 버튼이 있으면 한 클릭으로 열어요.

이 파일들을 클라우드 GPU 인스턴스에 *rsync / scp로 직접 업로드*해도 되고, 인스턴스에서 HF Hub로 다시 받아도 돼요. 후자가 깔끔.

---

## 3. 클라우드 GPU 옵션 (2026-05 기준)

| 서비스 | GPU 옵션 | 시간당 (USD) | 특징 |
|---|---|---|---|
| **RunPod Community** | RTX 4090 / 3090 | ~$0.40~0.70 | 가장 가벼움. 분 단위 과금 |
| **RunPod Secure Cloud** | A6000 / L40S | ~$0.79~1.49 | 안정. 24/7 운영 |
| **Vast.ai** | 4090 / 3090 | ~$0.30~0.50 | 가장 저렴. 안정성은 호스트 따라 다름 |
| **Lambda Labs** | A10 24GB | ~$0.75 | 안정 + 빠른 부팅 |
| **AWS g5.xlarge** | A10G | ~$1.00 | 엔터프라이즈 |
| **GCP a2-highgpu-1g** | A100 40GB | ~$3.67 | 큰 모델 (70B+) |

**베타 테스트 권장**: RunPod Community 4090 (~$0.50/h) → 일 8시간 운용 시 월 ~$120.

---

## 4. RunPod에서 llama.cpp 서버 띄우기 (예시)

### 4.1 인스턴스 생성

1. https://www.runpod.io 가입 + 잔액 충전.
2. "GPU Pods" → "Deploy" → RTX 4090 선택.
3. Template: **PyTorch 2.x + CUDA 12.x** (llama.cpp 빌드용).
4. Volume: 50GB+ (모델 파일 보관).
5. Expose HTTP Port: `8080`.
6. Deploy.

### 4.2 Web Terminal에서 셋업

```bash
# 1. llama.cpp 빌드.
cd /workspace
git clone https://github.com/ggml-org/llama.cpp
cd llama.cpp
make GGML_CUDA=1 -j$(nproc)

# 2. 모델 다운로드 (HF CLI).
pip install huggingface_hub
huggingface-cli download Qwen/Qwen3-30B-A3B-Instruct-GGUF \
  qwen3-30b-a3b-instruct-q4_k_m.gguf \
  --local-dir /workspace/models

# 3. 서버 시작 — OpenAI 호환 endpoint.
./llama-server \
  -m /workspace/models/qwen3-30b-a3b-instruct-q4_k_m.gguf \
  --host 0.0.0.0 \
  --port 8080 \
  --n-gpu-layers 999 \
  --ctx-size 8192 \
  --api-key "your-strong-secret-here"
```

### 4.3 외부 접속 — RunPod Public URL

RunPod 인스턴스 페이지에 자동으로 public URL이 발급돼요:
- 예: `https://abc123-8080.proxy.runpod.net`

웹앱에서:
```
Base URL: https://abc123-8080.proxy.runpod.net/v1
헤더:     Authorization: Bearer your-strong-secret-here
모델 ID:  qwen3-30b-a3b-instruct-q4_k_m.gguf
         (또는 llama-server가 노출하는 정확한 model id — /v1/models 호출로 확인)
```

5G 폰에서도 이 URL로 호출 가능 ✅.

---

## 5. Vast.ai에서 띄우기 (저렴 옵션)

### 5.1 인스턴스 검색

1. https://vast.ai → "Console" → "Search Templates" → `llama.cpp`.
2. 4090 / 3090 인스턴스 정렬 (가격 / GPU 메모리).
3. **Reliability ≥ 95%** + **DLPerf score 높은 호스트** 선택 (호스트 다운 위험 낮춤).
4. "Rent".

### 5.2 SSH로 접속 후 셋업

```bash
ssh -p PORT root@INSTANCE_IP
cd /workspace
# (RunPod 4.2와 동일한 빌드/실행 흐름)
```

### 5.3 Public URL 노출

Vast.ai는 RunPod 같은 자동 프록시 부재 — 두 옵션:

**옵션 A: ngrok 안에서**:
```bash
# 인스턴스 안에서.
curl -O https://bin.equinox.io/c/bNyj1mQVY4c/ngrok-v3-stable-linux-amd64.tgz
tar xzf ngrok-v3-stable-linux-amd64.tgz
./ngrok authtoken YOUR_TOKEN
./ngrok http 8080
```

**옵션 B: Vast.ai port 매핑**:
- 인스턴스 생성 시 `-p 8080:8080` 옵션 + Vast.ai의 public IP 사용.

---

## 6. 보안 체크리스트

✅ **API 키 강제** — `--api-key` 옵션. 16자 이상 랜덤 문자열.
✅ **HTTPS 엔드포인트** — RunPod proxy / cloudflared / nginx-letsencrypt 중 하나.
✅ **Rate limit** — nginx의 `limit_req` 또는 cloudflare WAF로 abuse 방지.
✅ **Origin 화이트리스트** — 웹앱 도메인만 통과. nginx `Origin` 헤더 검사 또는 cloudflare 룰.
✅ **로그 감시** — `/v1/chat/completions` 호출량 모니터링. 갑자기 급증하면 키 유출 의심.

⚠️ **하지 말 것**:
- `--api-key ""` (빈 키) — 인터넷 누구나 호출 가능.
- 게임 회사 PC에서 회사 데이터 학습된 모델 업로드.
- 인스턴스 종료 시 모델 파일 백업 안 한 채 떠나기 (스토리지 청구 계속).

---

## 7. 비용 절감 팁

- **사용 시간만 켜기**: RunPod / Vast.ai는 분 단위 과금. 베타 테스트 일 4시간이면 월 $60 수준.
- **모델 크기 다이어트**: 30B Q4_K_M 대신 14B Q5_K_M가 더 빠르고 한국어도 비슷.
- **Spot instances**: Vast.ai의 "Interruptible" 옵션은 50%+ 할인. 비핵심 테스트엔 충분.
- **이미지 캐싱**: 인스턴스 종료 시 Docker image 만들어두면 다음 부팅 빠름.

---

## 8. 트러블슈팅

| 증상 | 원인 후보 | 해결 |
|---|---|---|
| 빌드 시 CUDA 못 찾음 | nvcc PATH 누락 | `nvidia-smi`로 GPU 확인 후 `export PATH=/usr/local/cuda/bin:$PATH` |
| OOM during load | VRAM 부족 | `--n-gpu-layers`를 줄여 일부 layer를 CPU offload |
| `/v1/models`에 모델 안 뜸 | API key mismatch | `Authorization: Bearer <key>` 헤더 확인 |
| 5G 폰에서 timeout | 인스턴스 다운 / proxy 끊김 | RunPod 페이지 → "Restart" 또는 새 인스턴스 |
| TTFT(첫 토큰 시간) 너무 김 | cold start | 첫 요청은 항상 5~10초 소요 — warmup 호출 1회 후 본 호출 |

---

## 9. LMmaster와의 비교 정리

| 항목 | LMmaster (자택 PC) | 클라우드 GPU |
|---|---|---|
| 비용 | 전기료 + 1회 GPU | 시간당 $0.30~$1.00 |
| 5G/외부 접근 | ❌ (cloudflared 등 필요) | ✅ 자동 public URL |
| 한국어 큐레이션 | ✅ 카탈로그 + 추천 + 벤치 | ❌ 사용자가 결정 |
| 운영 부담 | 0 (그냥 띄워둠) | OS·llama.cpp·터널 셋업 |
| 데이터 위치 | 디스크 로컬 | 클라우드 (외부 기업) |
| 적합 용도 | 개인 사용 + 결정 도구 | 베타 테스트 + 외부 사용자 |

**권장 운용**: LMmaster를 *결정 도구*로 쓰고, 베타 테스트만 클라우드로 분리. 사내 자료를 클라우드로 옮기지 마세요.

---

## 참고

- ADR-0066: LAN 게이트웨이 노출 + API 키 UX 개편 (본 가이드의 모태).
- llama.cpp 공식: https://github.com/ggml-org/llama.cpp
- HuggingFace GGUF 카탈로그: https://huggingface.co/models?library=gguf
- RunPod docs: https://docs.runpod.io
- Vast.ai docs: https://vast.ai/faq
