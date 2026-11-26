import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Persona { id: string; engine: string; name: string; color: string; bound: boolean }
interface Room {
  id: string; kind: string; name: string; members: string[]; cwd: string;
  permission: string; default_target: string;
  last_message?: string | null; last_at?: string | null; last_sender?: string | null;
}
interface Msg {
  id: number; sender: string; role: string; text: string; run_id?: string | null;
  created_at: string; status?: string | null; tokens?: number | null;
  duration_s?: number | null; session_id?: string | null;
}
interface Doctor { tools: Record<string, string | null>; auth: Record<string, boolean>; mesh_root: string; path: string }

type View = { name: "bind" } | { name: "home" } | { name: "chat"; room: Room };

export default function App() {
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [view, setView] = useState<View>({ name: "home" });
  const [booted, setBooted] = useState(false);

  const refreshPersonas = useCallback(async () => {
    const ps = await invoke<Persona[]>("list_personas");
    setPersonas(ps);
    return ps;
  }, []);

  useEffect(() => {
    (async () => {
      const ps = await refreshPersonas();
      setView(ps.length === 0 ? { name: "bind" } : { name: "home" });
      setBooted(true);
    })();
  }, [refreshPersonas]);

  if (!booted) return <div className="boot">🐈‍⬛ AgentChat 启动中…</div>;
  if (view.name === "bind")
    return <BindWizard onDone={async () => { await refreshPersonas(); setView({ name: "home" }); }} />;
  if (view.name === "chat")
    return <ChatView room={view.room} personas={personas} onBack={() => setView({ name: "home" })} />;
  return <Home personas={personas} onOpenRoom={(room) => setView({ name: "chat", room })}
    onBindMore={() => setView({ name: "bind" })} />;
}

/* ---------- 绑定向导（M1：检测 8 CLI + auth → 勾选绑定） ---------- */

function BindWizard({ onDone }: { onDone: () => void }) {
  const [doctor, setDoctor] = useState<Doctor | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    invoke<Doctor>("doctor").then((d) => {
      setDoctor(d);
      // 默认勾选 CLI 存在的 engine
      const avail = Object.entries(d.tools)
        .filter(([k, v]) => v !== null && ["dsh","codex","opencode","hermes","arkcli","qwen","pi","claude"].includes(k))
        .map(([k]) => k);
      setSelected(new Set(avail));
    });
  }, []);

  const engines = ["dsh","codex","opencode","hermes","arkcli","qwen","pi","claude"];
  const labels: Record<string, string> = {
    dsh: "DeepSeek Harness", codex: "Codex", opencode: "OpenCode", hermes: "Hermes",
    arkcli: "豆包 arkcli", qwen: "千问", pi: "Pi", claude: "Claude Code",
  };

  const toggle = (e: string) =>
    setSelected((s) => { const n = new Set(s); n.has(e) ? n.delete(e) : n.add(e); return n; });

  return (
    <div className="wizard">
      <div className="logo">🐈‍⬛</div>
      <h1>绑定你的本地 Agent</h1>
      <p className="sub">检测到的 CLI 已自动勾选；未安装/未登录的可稍后补绑。它们将成为你的联系人。</p>
      <div className="engine-list">
        {engines.map((e) => {
          const path = doctor?.tools?.[e];
          return (
            <label key={e} className={`engine ${path ? "" : "disabled"}`}>
              <input type="checkbox" checked={selected.has(e)} disabled={!path} onChange={() => toggle(e)} />
              <span className="ename">{labels[e]}</span>
              <span className="estat">{path ? "✅ 已检测" : "⬜ 未安装"}</span>
            </label>
          );
        })}
      </div>
      <button className="primary" disabled={busy || selected.size === 0}
        onClick={async () => { setBusy(true); await invoke("bind_personas", { engines: [...selected] }); onDone(); }}>
        {busy ? "绑定中…" : `绑定 ${selected.size} 个 Agent，进入圆桌派 →`}
      </button>
    </div>
  );
}

/* ---------- 首页：会话列表（微信式） + 联系人 ---------- */

function Home({ personas, onOpenRoom, onBindMore }:
  { personas: Persona[]; onOpenRoom: (r: Room) => void; onBindMore: () => void }) {
  const [rooms, setRooms] = useState<Room[]>([]);
  const refresh = useCallback(() => invoke<Room[]>("list_rooms").then(setRooms), []);
  useEffect(() => { refresh(); }, [refresh]);

  const personaOf = (id: string) => personas.find((p) => p.id === id);

  return (
    <div className="home">
      <header className="topbar">
        <span className="brand">🐈‍⬛ 圆桌派</span>
        <button className="link" onClick={onBindMore}>＋ 绑定 Agent</button>
      </header>

      <section className="section-title">会话</section>
      {rooms.length === 0 && <div className="empty">还没有会话。从下方联系人发起私聊，或建一个群。</div>}
      {rooms.map((r) => {
        const first = r.members[0] ? personaOf(r.members[0]) : undefined;
        return (
          <div key={r.id} className="room" onClick={() => onOpenRoom(r)}>
            <Avatar name={r.name} color={first?.color ?? "#8e8e93"} group={r.kind === "group"} />
            <div className="room-main">
              <div className="room-name">{r.name}</div>
              <div className="room-last">{r.last_message ?? "（空会话）"}</div>
            </div>
            <div className="room-at">{r.last_at?.slice(5, 16).replace("T", " ") ?? ""}</div>
          </div>
        );
      })}

      <section className="section-title">联系人 · {personas.length}</section>
      <div className="contacts">
        {personas.map((p) => (
          <div key={p.id} className="contact">
            <Avatar name={p.name} color={p.color} />
            <div className="room-main">
              <div className="room-name">{p.name}</div>
              <div className="room-last">{p.engine}</div>
            </div>
            <button className="link" onClick={async () => {
              const room = await invoke<Room>("open_dm", { personaId: p.id });
              onOpenRoom(room);
            }}>私聊 →</button>
          </div>
        ))}
      </div>
    </div>
  );
}

/* ---------- 私聊对话框 ---------- */

function ChatView({ room, personas, onBack }:
  { room: Room; personas: Persona[]; onBack: () => void }) {
  const [history, setHistory] = useState<Msg[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  const refresh = useCallback(async () => {
    setHistory(await invoke<Msg[]>("room_history", { roomId: room.id }));
  }, [room.id]);
  useEffect(() => { refresh(); }, [refresh]);
  useEffect(() => { bottomRef.current?.scrollIntoView({ behavior: "smooth" }); }, [history, sending]);

  const nameOf = (sender: string) =>
    sender === "user" ? "我" : personas.find((p) => p.id === sender)?.name ?? sender;
  const colorOf = (sender: string) =>
    sender === "user" ? "#0a84ff" : personas.find((p) => p.id === sender)?.color ?? "#8e8e93";

  const send = async () => {
    const text = input.trim();
    if (!text || sending) return;
    setInput(""); setSending(true);
    try {
      const h = await invoke<Msg[]>("send_message", { roomId: room.id, text });
      setHistory(h);
    } finally { setSending(false); }
  };

  return (
    <div className="chat">
      <header className="topbar">
        <button className="link" onClick={onBack}>← 返回</button>
        <span className="brand">{room.name}</span>
        <span className="perm">{room.permission}</span>
      </header>
      <div className="bubbles">
        {history.map((m) => (
          <div key={m.id} className={`bubble-row ${m.role === "user" ? "me" : ""}`}>
            {m.role !== "user" && <Avatar name={nameOf(m.sender)} color={colorOf(m.sender)} small />}
            <div className={`bubble ${m.role === "user" ? "me" : ""}`}
              style={m.role !== "user" ? { borderLeftColor: colorOf(m.sender) } : {}}>
              <div className="bubble-head">
                <span style={{ color: colorOf(m.sender) }}>{nameOf(m.sender)}</span>
                <span className="meta">
                  {m.tokens != null && `⛁ ${m.tokens.toLocaleString()} tok · `}
                  {m.duration_s != null && `${Math.round(m.duration_s)}s · `}
                  {m.status && m.status !== "success" && <b className="bad">{m.status}</b>}
                  {m.created_at.slice(11, 19)}
                </span>
              </div>
              <pre className="bubble-text">{m.text}</pre>
            </div>
          </div>
        ))}
        {sending && <div className="typing">对方正在输入…</div>}
        <div ref={bottomRef} />
      </div>
      <div className="composer">
        <textarea value={input} rows={2} placeholder="发消息…（Enter 发送，Shift+Enter 换行）"
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); send(); } }} />
        <button className="primary" disabled={sending || !input.trim()} onClick={send}>
          {sending ? "…" : "发送"}
        </button>
      </div>
    </div>
  );
}

function Avatar({ name, color, group, small }:
  { name: string; color: string; group?: boolean; small?: boolean }) {
  return (
    <div className={`avatar ${small ? "small" : ""}`} style={{ background: color }}>
      {group ? "群" : name.slice(0, 1)}
    </div>
  );
}
