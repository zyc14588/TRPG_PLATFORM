"use client";

import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { Suspense, useEffect, useRef, useState } from "react";
import { apiErrorMessage, createTrpgApiClient } from "../../../lib/backend";

export default function AuthCallbackPage() {
  return (
    <Suspense fallback={<CallbackShell message="正在读取登录参数..." />}>
      <AuthCallbackInner />
    </Suspense>
  );
}

function AuthCallbackInner() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const [message, setMessage] = useState("正在验证 Magic Link...");
  const [error, setError] = useState<string | null>(null);
  const started = useRef(false);

  useEffect(() => {
    if (started.current) {
      return;
    }
    started.current = true;

    const token = searchParams.get("token");
    if (!token) {
      setError("Magic Link 缺少 token。");
      setMessage("无法完成登录。");
      return;
    }

    createTrpgApiClient()
      .verifyMagicLink(token)
      .then(() => {
        setMessage("登录成功，正在进入房间列表...");
        router.replace("/rooms");
      })
      .catch((err: unknown) => {
        setError(apiErrorMessage(err));
        setMessage("无法完成登录。");
      });
  }, [router, searchParams]);

  return <CallbackShell error={error} message={message} />;
}

function CallbackShell({ error, message }: { error?: string | null; message: string }) {
  return (
    <main className="page page-narrow">
      <section className="panel">
        <p className="eyebrow">Auth callback</p>
        <h1>{message}</h1>
        {error ? <p className="error">{error}</p> : <p className="status">请稍候。</p>}
        <Link className="button" href="/login">
          返回登录
        </Link>
      </section>
    </main>
  );
}
