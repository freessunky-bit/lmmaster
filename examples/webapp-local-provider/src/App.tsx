// 기존 웹앱의 채팅 화면이 LocalCompanionProvider를 어떻게 사용하는지 보여주는 미니 데모.
// 화면/메시지 형식/스트리밍 처리는 기존 웹앱 코드 그대로. provider 1개 추가가 변경의 전부.

import { useEffect, useState } from "react";
import { LocalCompanionProvider } from "./providers/local-companion";

const provider = new LocalCompanionProvider({
  apiKey: "<발급받은 로컬 키>",
});

export default function App() {
  const [available, setAvailable] = useState<boolean | null>(null);
  const [launchUrl, setLaunchUrl] = useState<string | undefined>();

  useEffect(() => {
    provider.ensureAvailable().then((r) => {
      setAvailable(r.ok);
      setLaunchUrl(r.launchUrl);
    });
  }, []);

  if (available === false) {
    return (
      <div>
        <p>LMmaster를 설치하거나 실행해주세요.</p>
        {launchUrl && <a href={launchUrl}>LMmaster 실행</a>}
      </div>
    );
  }
  return <div>연결됨. 채팅 UI는 여기에…</div>;
}
