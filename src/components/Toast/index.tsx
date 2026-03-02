import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

export function Toast() {
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<string>("clipboard-fallback", () => {
      const isMac = navigator.platform.toUpperCase().includes("MAC");
      setMessage(`Text copied — paste with ${isMac ? "Cmd" : "Ctrl"}+V`);
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  useEffect(() => {
    if (!message) return;
    const timer = setTimeout(() => setMessage(null), 4000);
    return () => clearTimeout(timer);
  }, [message]);

  if (!message) return null;

  return (
    <div
      style={{
        position: "fixed",
        bottom: 20,
        left: "50%",
        transform: "translateX(-50%)",
        padding: "8px 16px",
        borderRadius: 8,
        background: "var(--bg-surface)",
        border: "1px solid var(--border)",
        color: "var(--text-primary)",
        fontSize: 13,
        fontFamily: "var(--font-sans)",
        boxShadow: "0 2px 8px rgba(0,0,0,0.12)",
        zIndex: 1000,
        animation: "toast-in 0.2s ease-out",
      }}
    >
      {message}
    </div>
  );
}
