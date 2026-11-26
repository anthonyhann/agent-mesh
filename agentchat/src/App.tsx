import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Persona { id: string; engine: string; name: string; color: string; bound: boolean }
interface Room {
  id: string; kind: string; name: string; members: string[]; cwd: string;
  permission: string; reply_mode: string;
  last_message?: string | null; last_at?: string | null; last_sender?: string | null;
}
interface Msg {
  id: number; sender: string; role: string; text: string;
  turn_id?: string | null; run_id?: string | null; client_msg_id?: string | null;
  created_at: string; status?: string | null; tokens?: number | null;
  duration_s?: number | null; session_id?: string | null;
}
interface Doctor { tools: Record<string, string | null>; auth: Record<string, boolean>; mesh_root: string; path: string }

export default function App() {
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [rooms, setRooms] = useState<Room[]>([]);
  const [activeRoom, setActiveRoom] = useState<Room | null>(null);
  const [tab, setTab] = useState<"chat" | "contacts">("chat");
  const [showBind, setShowBind] = useState(false);
  const [booted, setBooted] = useState(false);

  const refreshPersonas = useCallback(async () => {
    const ps = await invoke<Persona[]>("list_personas");
    setPersonas(ps);
    return ps;
  }, []);
  const refreshRooms = useCallback(async () => {
    const rs = await invoke<Room[]>("list_rooms");
    setRooms(rs);
    return rs;
  }, []);

  useEffect(() => {
    (async () => {
      const ps = await refreshPersonas();
      await refreshRooms();
      setShowBind(ps.length === 0);
      setBooted(true);
    })();
  }, [refreshPersonas, refreshRooms]);

  const openDm = useCallback(async (personaId: string) => {
    const room = await invoke<Room>("open_dm", { personaId });
    await refreshRooms();
    setActiveRoom(room);
    setTab("chat");
  }, [refreshRooms]);

  if (!booted) return <div className="boot">🐈‍⬛ AgentChat 启动中…</div>;

  return (
    <div className="app">
      {/* 左：icon rail（WhatsApp 式：顶头像、中间导航、底部动作） */}
      <nav className="rail">
        <div className="rail-avatar" title="我">H</div>
        <button className={`rail-btn ${tab === "chat" ? "on" : ""}`} title="聊天" onClick={() => setTab("chat")}>💬</button>
        <button className={`rail-btn ${tab === "contacts" ? "on" : ""}`} title="联系人" onClick={() => setTab("contacts")}>🤖</button>
        <div className="rail-spacer" />
        <button className="rail-btn" title="绑定 Agent" onClick={() => setShowBind(true)}>➕</button>
      </nav>

      {/* 中：列表栏 */}
      <aside className="sidebar">
        {tab === "chat" ? (
          <SessionList rooms={rooms} personas={personas} activeId={activeRoom?.id ?? null}
            onSelect={(r) => setActiveRoom(r)} onBind={() => setShowBind(true)}
            onCompose={() => setTab("contacts")} />
        ) : (
          <ContactList personas={personas} onDm={openDm} onBind={() => setShowBind(true)} />
        )}
      </aside>

      {/* 右：聊天区 */}
      <main className="main">
        {activeRoom ? (
          <ChatView key={activeRoom.id} room={activeRoom} personas={personas}
            onSent={refreshRooms} />
        ) : (
          <div className="placeholder">
            <div className="ph-logo">🐈‍⬛</div>
            <div className="ph-title">圆桌派 AgentChat</div>
            <div className="ph-text">本地 CLI Agent 都是你的联系人<br />选择左侧会话，或从联系人发起私聊</div>
          </div>
        )}
      </main>

      {showBind && (
        <BindWizard onClose={async (done) => {
          setShowBind(false);
          if (done) { await refreshPersonas(); await refreshRooms(); }
        }} />
      )}
    </div>
  );
}

/* ---------- 会话列表 ---------- */

function SessionList({ rooms, personas, activeId, onSelect, onBind, onCompose }:
  { rooms: Room[]; personas: Persona[]; activeId: string | null; onSelect: (r: Room) => void; onBind: () => void; onCompose: () => void }) {
  const [q, setQ] = useState("");
  const filtered = rooms.filter((r) =>
    !q || r.name.toLowerCase().includes(q.toLowerCase()) || (r.last_message ?? "").toLowerCase().includes(q.toLowerCase()));
  const personaOf = (id: string) => personas.find((p) => p.id === id);

  return (
    <>
      <div className="sidebar-head">
        <span className="sb-title">会话</span>
        <button className="icon-btn" title="发起私聊" onClick={onCompose}>✎</button>
      </div>
      <div className="search"><input placeholder="搜索会话" value={q} onChange={(e) => setQ(e.target.value)} /></div>
      <div className="list">
        <div className="add-agent" onClick={onBind}>
          <div className="add-icon">➕</div><div className="add-text">添加 Agent</div>
        </div>
        {filtered.length === 0 && <div className="empty">还没有会话</div>}
        {filtered.map((r) => {
          const first = r.members[0] ? personaOf(r.members[0]) : undefined;
          return (
            <div key={r.id} className={`conv ${activeId === r.id ? "on" : ""}`} onClick={() => onSelect(r)}>
              <Avatar name={r.name} color={first?.color ?? "#8696a0"} group={r.kind === "group"} />
              <div className="conv-main">
                <div className="conv-top">
                  <span className="conv-name">{r.name}</span>
                  <span className="conv-at">{fmtListTime(r.last_at)}</span>
                </div>
                <div className="conv-last">{previewOf(r)}</div>
              </div>
            </div>
          );
        })}
      </div>
    </>
  );
}

function previewOf(r: Room): string {
  if (!r.last_message) return "（空会话）";
  const oneLine = r.last_message.replace(/\n/g, " ");
  return oneLine.length > 60 ? oneLine.slice(0, 60) + "…" : oneLine;
}

function fmtListTime(at?: string | null): string {
  if (!at) return "";
  const d = new Date(at);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  if (sameDay) return `${hh}:${mm}`;
  return `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/* ---------- 联系人 ---------- */

function ContactList({ personas, onDm, onBind }:
  { personas: Persona[]; onDm: (id: string) => void; onBind: () => void }) {
  return (
    <>
      <div className="sidebar-head"><span className="sb-title">联系人 · {personas.length}</span></div>
      <div className="list">
        <div className="add-agent" onClick={onBind}>
          <div className="add-icon">➕</div><div className="add-text">添加 Agent</div>
        </div>
        {personas.map((p) => (
          <div key={p.id} className="conv" onClick={() => onDm(p.id)}>
            <Avatar name={p.name} color={p.color} />
            <div className="conv-main">
              <div className="conv-top"><span className="conv-name">{p.name}</span></div>
              <div className="conv-last">{p.engine} · 在线</div>
            </div>
            <span className="online-dot" />
          </div>
        ))}
      </div>
    </>
  );
}

/* ---------- 绑定向导（弹层） ---------- */

function BindWizard({ onClose }: { onClose: (done: boolean) => void }) {
  const [doctor, setDoctor] = useState<Doctor | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    invoke<Doctor>("doctor").then((d) => {
      setDoctor(d);
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
    <div className="modal-mask" onClick={() => onClose(false)}>
      <div className="wizard" onClick={(e) => e.stopPropagation()}>
        <div className="logo">🐈‍⬛</div>
        <h1>绑定你的本地 Agent</h1>
        <p className="sub">检测到的 CLI 已自动勾选；未安装/未登录的可稍后补绑。</p>
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
          onClick={async () => { setBusy(true); await invoke("bind_personas", { engines: [...selected] }); onClose(true); }}>
          {busy ? "绑定中…" : `绑定 ${selected.size} 个 Agent →`}
        </button>
      </div>
    </div>
  );
}

/* ---------- 聊天区 ---------- */

function ChatView({ room, personas, onSent }:
  { room: Room; personas: Persona[]; onSent: () => void }) {
  const [history, setHistory] = useState<Msg[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);

  const refresh = useCallback(async () => {
    setHistory(await invoke<Msg[]>("room_history", { roomId: room.id }));
  }, [room.id]);
  useEffect(() => { refresh(); }, [refresh]);
  useEffect(() => { bottomRef.current?.scrollIntoView({ behavior: "smooth" }); }, [history, sending]);

  const nameOf = (sender: string) =>
    sender === "user" ? "我" : personas.find((p) => p.id === sender)?.name ?? sender;
  const colorOf = (sender: string) =>
    personas.find((p) => p.id === sender)?.color ?? "#8696a0";

  // client_msg_id：每次发送自铸，重试沿用 → UNIQUE(room_id, client_msg_id) 幂等（§4.11）
  const pendingIdRef = useRef<string | null>(null);
  const send = async () => {
    const text = input.trim();
    if (!text || sending) return;
    if (!pendingIdRef.current) pendingIdRef.current = mintClientMsgId();
    setInput(""); setSending(true);
    if (taRef.current) taRef.current.style.height = "";
    try {
      const h = await invoke<Msg[]>("send_message", { roomId: room.id, text, clientMsgId: pendingIdRef.current });
      setHistory(h);
      pendingIdRef.current = null;
      onSent();
    } catch {
      setInput(text); // 失败回填，重试沿用同一 client_msg_id
    } finally { setSending(false); }
  };

  const grow = () => {
    const t = taRef.current;
    if (t) { t.style.height = "0"; t.style.height = Math.min(t.scrollHeight, 132) + "px"; }
  };

  const firstPersona = room.members[0] ? personas.find((p) => p.id === room.members[0]) : undefined;

  return (
    <div className="chat">
      <header className="chat-head">
        {firstPersona && <Avatar name={firstPersona.name} color={firstPersona.color} small />}
        <div className="ch-meta">
          <div className="ch-name">{room.name}</div>
          <div className="ch-status"><span className="online-dot" />在线 · {room.permission}</div>
        </div>
        <div className="ch-actions">
          <button className="icon-btn" title="搜索聊天记录（即将上线）" disabled>🔍</button>
          <button className="icon-btn" title="更多（即将上线）" disabled>⋯</button>
        </div>
      </header>
      <div className="bubbles">
        {history.map((m, i) => {
          const prev = history[i - 1];
          const divider = needDivider(prev, m);
          const me = m.role === "user";
          // 分组：同发送者、无分隔 → 后续消息紧凑（省头像/名字）
          const first = !prev || prev.sender !== m.sender || divider;
          const answered = me && history.some((x, j) => j > i && x.role !== "user");
          return (
            <div key={m.id}>
              {divider && <div className="time-divider"><span>{fmtDivider(m.created_at)}</span></div>}
              <div className={`bubble-row ${me ? "me" : ""} ${first ? "" : "compact"}`}>
                {!me && (first
                  ? <Avatar name={nameOf(m.sender)} color={colorOf(m.sender)} small />
                  : <div className="avatar-sp" />)}
                <div className={`bubble ${me ? "me" : ""} ${first ? "first" : ""}`}>
                  {!me && first && room.kind === "group" && (
                    <div className="b-name" style={{ color: colorOf(m.sender) }}>{nameOf(m.sender)}</div>
                  )}
                  <pre className="bubble-text">{m.text}</pre>
                  <div className="b-foot">
                    {m.tokens != null && <span className="b-meta">⛁ {fmtTok(m.tokens)}</span>}
                    {m.duration_s != null && <span className="b-meta">{Math.round(m.duration_s)}s</span>}
                    {m.status && m.status !== "success" && <span className="bad">{m.status}</span>}
                    <span className="b-time">{hm(m.created_at)}</span>
                    {me && <span className={`ticks ${answered ? "read" : ""}`}>{answered ? "✓✓" : "✓"}</span>}
                  </div>
                </div>
                {me && (first ? <div className="avatar small me-avatar">H</div> : <div className="avatar-sp" />)}
              </div>
            </div>
          );
        })}
        {sending && (
          <div className="bubble-row">
            {firstPersona && <Avatar name={firstPersona.name} color={firstPersona.color} small />}
            <div className="bubble first typing"><span /><span /><span /></div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>
      <div className="composer">
        <div className="composer-pill">
          <button className="tool" title="表情（即将上线）" disabled>😊</button>
          <button className="tool" title="快捷指令（即将上线）" disabled>⚡</button>
          <button className="tool" title="图片/文件（即将上线）" disabled>📎</button>
          <textarea ref={taRef} value={input} rows={1} placeholder={`发送给 ${room.name}`}
            onChange={(e) => { setInput(e.target.value); grow(); }}
            onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); send(); } }} />
        </div>
        <button className="send" disabled={sending || !input.trim()} onClick={send}>➤</button>
      </div>
    </div>
  );
}

function needDivider(prev: Msg | undefined, cur: Msg): boolean {
  if (!prev) return true;
  const gap = new Date(cur.created_at).getTime() - new Date(prev.created_at).getTime();
  return gap > 5 * 60 * 1000;
}
function fmtDivider(at: string): string {
  const d = new Date(at);
  const now = new Date();
  const hm = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  if (d.toDateString() === now.toDateString()) return hm;
  return `${d.getMonth() + 1}月${d.getDate()}日 ${hm}`;
}
function hm(at: string): string {
  const d = new Date(at);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}
function fmtTok(n: number): string {
  return n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n);
}
function mintClientMsgId(): string {
  // msg-<ulid 风格>：时间有序 + 随机，26 字符 Crockford base32（与后端 §4.11 同形）
  const CROCK = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
  const rand = crypto.getRandomValues(new Uint8Array(10));
  let v = (BigInt(Date.now()) & 0xffffffffffffn) << 80n;
  rand.forEach((b, i) => { v |= BigInt(b) << BigInt((9 - i) * 8); });
  let s = "";
  for (let i = 0; i < 26; i++) s += CROCK[Number((v >> BigInt(125 - 5 * i)) & 31n)];
  return `msg-${s}`;
}

function Avatar({ name, color, group, small }:
  { name: string; color: string; group?: boolean; small?: boolean }) {
  return (
    <div className={`avatar ${small ? "small" : ""}`} style={{ background: color }}>
      {group ? "群" : name.slice(0, 1)}
    </div>
  );
}
