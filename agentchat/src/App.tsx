import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface Persona { id: string; engine: string; name: string; color: string; bound: boolean; behavior_md?: string }
interface PersonaDetail extends Persona {
  session_id?: string | null; sandbox?: string | null;
  grants_badge: string; credential_ok: boolean; credential_hint?: string | null;
}
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

/* ---------- SF Symbols 风格图标（细描边圆角内联 SVG） ---------- */

function Icon({ children, size = 19 }: { children: ReactNode; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none"
      stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      {children}
    </svg>
  );
}
const I = {
  chat: <path d="M21 11.5a8.5 8.5 0 0 1-8.5 8.5c-1.4 0-2.7-.3-3.8-.9L3 21l1.9-5.7a8.5 8.5 0 1 1 16.1-3.8z" />,
  robot: <>
    <rect x="4.5" y="8" width="15" height="11" rx="3.2" />
    <path d="M12 4.5V8" /><path d="M2.5 12.5v3M21.5 12.5v3" />
    <circle cx="9.3" cy="13.3" r="1.15" fill="currentColor" stroke="none" />
    <circle cx="14.7" cy="13.3" r="1.15" fill="currentColor" stroke="none" />
  </>,
  plus: <path d="M12 5.5v13M5.5 12h13" />,
  compose: <>
    <path d="M12 20h8.5" />
    <path d="M16.4 3.6a2.1 2.1 0 0 1 3 3L7.2 18.8l-4 1 1-4L16.4 3.6z" />
  </>,
  search: <><circle cx="11" cy="11" r="6.5" /><path d="M20.5 20.5 15.7 15.7" /></>,
  more: <>
    <circle cx="5.5" cy="12" r="1.3" fill="currentColor" stroke="none" />
    <circle cx="12" cy="12" r="1.3" fill="currentColor" stroke="none" />
    <circle cx="18.5" cy="12" r="1.3" fill="currentColor" stroke="none" />
  </>,
  smile: <>
    <circle cx="12" cy="12" r="8.5" />
    <circle cx="9.1" cy="9.8" r="1.1" fill="currentColor" stroke="none" />
    <circle cx="14.9" cy="9.8" r="1.1" fill="currentColor" stroke="none" />
    <path d="M8.3 14.2c.9 1.3 2.1 2 3.7 2s2.8-.7 3.7-2" />
  </>,
  bolt: <path d="M13 2.5 4.5 14H11l-1 7.5L18.5 10H12l1-7.5z" />,
  clip: <path d="M20.8 11.3 12.3 19.8a5.4 5.4 0 0 1-7.6-7.6l9-9a3.6 3.6 0 0 1 5.1 5.1l-9 9a1.8 1.8 0 0 1-2.5-2.5l8.2-8.2" />,
  send: <path d="M12 18.5v-13M5.8 11.7 12 5.5l6.2 6.2" />,
  stop: <rect x="7" y="7" width="10" height="10" rx="2.5" fill="currentColor" stroke="none" />,
  copy: <><rect x="8.5" y="8.5" width="11" height="11" rx="2.5" /><path d="M15.5 8.5v-3a2.5 2.5 0 0 0-2.5-2.5h-7a2.5 2.5 0 0 0-2.5 2.5v11a2.5 2.5 0 0 0 2.5 2.5h3" /></>,
};

/* 权限徽标（与后端 grants_badge 同口径，列表/头部无需详情调用时用） */
function badgeOf(engine: string, sandbox?: string | null): string {
  switch (engine) {
    case "dsh": return "never";
    case "claude": return "bypass";
    case "qwen": return "yolo";
    case "codex": return `sandbox: ${sandbox ?? "workspace-write"}`;
    default: return "default";
  }
}

export default function App() {
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [rooms, setRooms] = useState<Room[]>([]);
  const [activeRoom, setActiveRoom] = useState<Room | null>(null);
  const [tab, setTab] = useState<"chat" | "contacts">("chat");
  const [showBind, setShowBind] = useState(false);
  const [booted, setBooted] = useState(false);
  // M2b：联系人名片弹层（persona_id）
  const [cardId, setCardId] = useState<string | null>(null);

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
// 多开的新 persona 也要进列表（否则聊天头部查不到徽标/引擎信息）
await Promise.all([refreshRooms(), refreshPersonas()]);
setActiveRoom(room);
setTab("chat");
}, [refreshRooms, refreshPersonas]);

  if (!booted) return <div className="boot">AgentChat 启动中…</div>;

  return (
    <div className="app">
      {/* 左：icon rail（macOS 式：顶头像、中导航、底动作） */}
      <nav className="rail">
        <div className="rail-avatar" title="我">H</div>
        <button className={`rail-btn ${tab === "chat" ? "on" : ""}`} title="聊天" onClick={() => setTab("chat")}><Icon>{I.chat}</Icon></button>
        <button className={`rail-btn ${tab === "contacts" ? "on" : ""}`} title="联系人" onClick={() => setTab("contacts")}><Icon>{I.robot}</Icon></button>
        <div className="rail-spacer" />
        <button className="rail-btn" title="绑定 Agent" onClick={() => setShowBind(true)}><Icon>{I.plus}</Icon></button>
      </nav>

      {/* 中：列表栏 */}
      <aside className="sidebar">
        {tab === "chat" ? (
          <SessionList rooms={rooms} personas={personas} activeId={activeRoom?.id ?? null}
            onSelect={(r) => setActiveRoom(r)} onBind={() => setShowBind(true)}
            onCompose={() => setTab("contacts")} />
        ) : (
          <ContactList personas={personas} onDm={openDm} onBind={() => setShowBind(true)}
            onCard={(id) => setCardId(id)} />
        )}
      </aside>

      {/* 右：聊天区 */}
      <main className="main">
        {activeRoom ? (
          <ChatView key={activeRoom.id} room={activeRoom} personas={personas}
            onSent={refreshRooms} onShowCard={(id) => setCardId(id)} />
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
      {cardId && (
        <PersonaCard personaId={cardId} roomId={activeRoom?.id ?? null}
          onClose={async () => { setCardId(null); await refreshPersonas(); await refreshRooms(); }}
          onDm={(id) => { setCardId(null); openDm(id); }} />
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
        <button className="icon-btn" title="发起私聊" onClick={onCompose}><Icon size={17}>{I.compose}</Icon></button>
      </div>
      <div className="search"><input placeholder="搜索" value={q} onChange={(e) => setQ(e.target.value)} /></div>
      <div className="list">
        <div className="add-agent" onClick={onBind}>
          <div className="add-icon"><Icon size={18}>{I.plus}</Icon></div><div className="add-text">添加 Agent</div>
        </div>
        {filtered.length === 0 && <div className="empty">还没有会话</div>}
        {filtered.map((r) => {
          const first = r.members[0] ? personaOf(r.members[0]) : undefined;
          return (
            <div key={r.id} className={`conv ${activeId === r.id ? "on" : ""}`} onClick={() => onSelect(r)}>
              <Avatar name={r.name} color={first?.color ?? "#98989f"} group={r.kind === "group"} />
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

function ContactList({ personas, onDm, onBind, onCard }:
  { personas: Persona[]; onDm: (id: string) => void; onBind: () => void; onCard: (id: string) => void }) {
  return (
    <>
      <div className="sidebar-head"><span className="sb-title">联系人 · {personas.length}</span></div>
      <div className="list">
        <div className="add-agent" onClick={onBind}>
          <div className="add-icon"><Icon size={18}>{I.plus}</Icon></div><div className="add-text">添加 Agent</div>
        </div>
        {personas.map((p) => (
          <div key={p.id} className="conv" onClick={() => onDm(p.id)}>
            <Avatar name={p.name} color={p.color} />
            <div className="conv-main">
              <div className="conv-top"><span className="conv-name">{p.name}</span></div>
              <div className="conv-last">{p.engine} · <span className="badge">{badgeOf(p.engine)}</span> · 在线</div>
            </div>
            <button className="icon-btn" title="名片"
              onClick={(e) => { e.stopPropagation(); onCard(p.id); }}><Icon size={15}>{I.more}</Icon></button>
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

function ChatView({ room, personas, onSent, onShowCard }:
  { room: Room; personas: Persona[]; onSent: () => void; onShowCard: (id: string) => void }) {
  const [history, setHistory] = useState<Msg[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  // M2a-3：typing 事件驱动（persona_id → 最后节拍时间），不再用本地 sending 模拟
  const [typing, setTyping] = useState<Record<string, number>>({});
  const bottomRef = useRef<HTMLDivElement>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);

  const refresh = useCallback(async () => {
    setHistory(await invoke<Msg[]>("room_history", { roomId: room.id }));
  }, [room.id]);
  useEffect(() => { refresh(); }, [refresh]);
  useEffect(() => { bottomRef.current?.scrollIntoView({ behavior: "smooth" }); }, [history, typing]);

  // M2a-3 异步事件：typing start/beat/end（C6 节奏）+ history（气泡入库刷新）
  useEffect(() => {
    let unTyping: (() => void) | undefined;
    let unHist: (() => void) | undefined;
    listen<{ room_id: string; persona_id: string; phase: string }>("ac://typing", (e) => {
      const p = e.payload;
      if (p.room_id !== room.id) return;
      setTyping((prev) => {
        const n = { ...prev };
        if (p.phase === "end") delete n[p.persona_id];
        else n[p.persona_id] = Date.now();
        return n;
      });
    }).then((u) => { unTyping = u; });
    listen<{ room_id: string }>("ac://history", (e) => {
      if (e.payload.room_id === room.id) { refresh(); onSent(); }
    }).then((u) => { unHist = u; });
    return () => { unTyping?.(); unHist?.(); };
  }, [room.id, refresh, onSent]);

  // C6：6s 无节拍自动熄灭（事件丢失兜底）
  useEffect(() => {
    const t = setInterval(() => {
      setTyping((prev) => {
        const now = Date.now();
        const n = Object.fromEntries(Object.entries(prev).filter(([, ts]) => now - ts < 6000));
        return Object.keys(n).length === Object.keys(prev).length ? prev : n;
      });
    }, 1000);
    return () => clearInterval(t);
  }, []);

  const nameOf = (sender: string) =>
    sender === "user" ? "我" : personas.find((p) => p.id === sender)?.name ?? sender;
  const colorOf = (sender: string) =>
    personas.find((p) => p.id === sender)?.color ?? "#98989f";

  // client_msg_id：每次发送自铸，重试沿用 → UNIQUE(room_id, client_msg_id) 幂等（§4.11）
  const pendingIdRef = useRef<string | null>(null);
  // M2a-3 异步：send_message 只入队（消息+turn 同事务）即返回自己的消息回显；
  // 执行由后端 lane 调度器消费，typing/气泡/取消均由事件驱动
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
  // §6.8 即时打断：停止当前回合（killpg → run cancelled → lane 继续消费下一 turn）
  const stop = async () => {
    await invoke("stop_turn", { roomId: room.id });
    setTyping({}); // 本地立即熄灭（后端 typing end 事件兜底）
  };

  const grow = () => {
    const t = taRef.current;
    if (t) { t.style.height = "0"; t.style.height = Math.min(t.scrollHeight, 132) + "px"; }
  };

  const firstPersona = room.members[0] ? personas.find((p) => p.id === room.members[0]) : undefined;
  // Messages 式回执：只挂最后一条我发的消息
  let lastUserIdx = -1;
  history.forEach((m, i) => { if (m.role === "user") lastUserIdx = i; });

  return (
    <div className="chat">
      <header className="chat-head">
        {firstPersona && <Avatar name={firstPersona.name} color={firstPersona.color} small />}
        <div className="ch-meta clickable" title="查看名片" onClick={() => firstPersona && onShowCard(firstPersona.id)}>
          <div className="ch-name">{room.name}</div>
          <div className="ch-status"><span className="online-dot" />在线 · {room.permission}
            {firstPersona && <span className="badge">{badgeOf(firstPersona.engine)}</span>}</div>
        </div>
        <div className="ch-actions">
          <button className="icon-btn" title="搜索聊天记录（即将上线）" disabled><Icon size={17}>{I.search}</Icon></button>
          <button className="icon-btn" title="名片" onClick={() => firstPersona && onShowCard(firstPersona.id)}><Icon size={17}>{I.more}</Icon></button>
        </div>
      </header>
      <div className="bubbles">
        {history.map((m, i) => {
          const prev = history[i - 1];
          const next = history[i + 1];
          const divider = needDivider(prev, m);
          const me = m.role === "user";
          // 分组：同发送者连续消息为一组；头像挂在组尾（iMessage 式），组尾气泡切角
          const first = !prev || prev.sender !== m.sender || divider;
          const last = !next || next.sender !== m.sender || needDivider(m, next);
          const answered = me && history.some((x, j) => j > i && x.role !== "user");
          return (
            <div key={m.id}>
              {divider && <div className="time-divider">{fmtDivider(m.created_at)}</div>}
              <div className={`bubble-row ${me ? "me" : ""} ${first ? "" : "compact"}`}>
                {!me && (last
                  ? <Avatar name={nameOf(m.sender)} color={colorOf(m.sender)} small />
                  : <div className="avatar-sp" />)}
                <div className={`bubble ${me ? "me" : ""} ${last ? "last" : ""}`}>
                  {!me && first && room.kind === "group" && (
                    <div className="b-name" style={{ color: colorOf(m.sender) }}>{nameOf(m.sender)}</div>
                  )}
                  <pre className="bubble-text">{m.text}</pre>
                  {/* M2b 失败引导：failed 泡给重发指引；skipped 泡正文已是配置指引 */}
                  {!me && m.status === "failed" && (
                    <div className="b-hint">发送失败：检查名片中的凭证状态，或点此消息上方重发</div>
                  )}
                  <div className="b-foot">
                    {m.tokens != null && <span className="b-meta">⛁ {fmtTok(m.tokens)}</span>}
                    {m.duration_s != null && <span className="b-meta">{Math.round(m.duration_s)}s</span>}
                    {m.status && m.status !== "success" && <span className="bad">{m.status}</span>}
                    <span className="b-time">{hm(m.created_at)}</span>
                  </div>
                </div>
                {me && (last ? <div className="avatar small me-avatar">H</div> : <div className="avatar-sp" />)}
              </div>
              {i === lastUserIdx && (
                <div className="receipt">{answered ? "已读" : "已送达"}</div>
              )}
            </div>
          );
        })}
        {Object.keys(typing).map((pid) => {
          const p = personas.find((x) => x.id === pid);
          return (
            <div key={pid} className="bubble-row">
              {p ? <Avatar name={p.name} color={p.color} small /> : <div className="avatar-sp" />}
              <div className="bubble last typing"><span /><span /><span /></div>
            </div>
          );
        })}
        <div ref={bottomRef} />
      </div>
      <div className="composer">
        <div className="composer-pill">
          <button className="tool" title="表情（即将上线）" disabled><Icon size={18}>{I.smile}</Icon></button>
          <button className="tool" title="快捷指令（即将上线）" disabled><Icon size={18}>{I.bolt}</Icon></button>
          <button className="tool" title="图片/文件（即将上线）" disabled><Icon size={18}>{I.clip}</Icon></button>
          <textarea ref={taRef} value={input} rows={1} placeholder={`发送给 ${room.name}`}
            onChange={(e) => { setInput(e.target.value); grow(); }}
            onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); send(); } }} />
        </div>
        {Object.keys(typing).length > 0 && (
          <button className="stop" title="停止当前回合" onClick={stop}><Icon size={13}>{I.stop}</Icon></button>
        )}
        <button className="send" disabled={sending || !input.trim()} onClick={send}><Icon size={17}>{I.send}</Icon></button>
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

/* ---------- M2b 联系人名片（弹层）：详情 + behavior.md 编辑 + 多开实例 ---------- */

function PersonaCard({ personaId, roomId, onClose, onDm }:
  { personaId: string; roomId: string | null; onClose: () => void; onDm: (id: string) => void }) {
  const [d, setD] = useState<PersonaDetail | null>(null);
  const [md, setMd] = useState("");
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    invoke<PersonaDetail>("persona_detail", { personaId, roomId }).then((v) => {
      setD(v);
      setMd(v.behavior_md ?? "");
    });
  }, [personaId, roomId]);

  const save = async () => {
    setBusy(true);
    await invoke("set_behavior", { personaId, behaviorMd: md });
    setBusy(false);
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  };
  // persona 多开（§4.5）：同 engine 新实例，自动编号命名
  const clone = async () => {
    if (!d) return;
    setBusy(true);
    const p = await invoke<Persona>("add_persona", { engine: d.engine });
    setBusy(false);
    onDm(p.id);
  };

  return (
    <div className="modal-mask" onClick={onClose}>
      <div className="wizard card" onClick={(e) => e.stopPropagation()}>
        {d ? (
          <>
            <div className="card-head">
              <Avatar name={d.name} color={d.color} />
              <div className="card-title">
                <div className="card-name">{d.name}</div>
                <div className="card-sub">{d.engine} · <span className="badge">{d.grants_badge}</span></div>
              </div>
              <span className={`cred ${d.credential_ok ? "ok" : "bad"}`}>
                {d.credential_ok ? "凭证 ✓" : "凭证 ✗"}
              </span>
            </div>
            {!d.credential_ok && d.credential_hint && <div className="cred-hint">{d.credential_hint}</div>}
            {d.session_id && (
              <div className="card-row">
                <span className="card-key">会话</span>
                <span className="card-val mono">{d.session_id.slice(0, 24)}{d.session_id.length > 24 ? "…" : ""}</span>
                {d.sandbox && <span className="badge">sandbox: {d.sandbox}</span>}
              </div>
            )}
            <div className="card-row col">
              <span className="card-key">behavior.md（身份/风格/何时不发言/篇幅/禁推理外露）</span>
              <textarea className="behavior" rows={6} value={md} onChange={(e) => setMd(e.target.value)}
                placeholder="例：你是代码评审专家。回复控制在 100 字内。不主动评价无关话题。" />
            </div>
            <div className="card-actions">
              <button className="primary" disabled={busy} onClick={save}>{saved ? "已保存 ✓" : "保存 behavior"}</button>
              <button className="ghost" disabled={busy} onClick={clone} title="同引擎再开一个实例（会话独立）">
                <Icon size={14}>{I.copy}</Icon> 再开一个实例
              </button>
            </div>
          </>
        ) : (
          <div className="empty">加载中…</div>
        )}
      </div>
    </div>
  );
}
