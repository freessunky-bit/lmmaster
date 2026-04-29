"""ML 워커 JSON-RPC 서버 (placeholder).

v1에서는 Rust supervisor가 이 프로세스를 띄우지 않는다.
v1.x 이후 실제 작업이 구현되면 활성.
"""

import sys
import json


def main() -> int:
    sys.stderr.write("[lmmaster-ml] placeholder server. Not active in v1.\n")
    sys.stderr.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())
