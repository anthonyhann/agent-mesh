//! AgentChat SQLite 层（§4.3 会话事实源）
//! DB 位于 MESH_DATA 顶层 agentchat.db；results/ 仅为不可变 artifact 仓库。

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::Path;

pub struct Db(pub Connection);

/// 8 个 engine 注册表：(engine, 显示名, 主题色)
pub const ENGINES: [(&str, &str, &str); 8] = [
    ("dsh", "DeepSeek Harness", "#0a84ff"),
    ("codex", "Codex", "#10a37f"),
    ("opencode", "OpenCode", "#f97316"),
    ("hermes", "Hermes", "#8b5cf6"),
    ("arkcli", "豆包 arkcli", "#ef4444"),
    ("qwen", "千问", "#615ced"),
    ("pi", "Pi", "#e11d48"),
    ("claude", "Claude Code", "#d97706"),
];

pub fn engine_label(engine: &str) -> &'static str {
    ENGINES
        .iter()
        .find(|(e, _, _)| *e == engine)
        .map(|(_, l, _)| *l)
        .unwrap_or("Unknown")
}

pub fn engine_color(engine: &str) -> &'static str {
    ENGINES
        .iter()
        .find(|(e, _, _)| *e == engine)
        .map(|(_, _, c)| *c)
        .unwrap_or("#8e8e93")
}

fn now() -> String {
    // ISO8601 UTC，不引 chrono：用 time crate 也太重，直接 libc
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::gmtime_r(&t, &mut tm);
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min,
            tm.tm_sec
        )
    }
}

pub fn uuid8() -> String {
    let mut b = [0u8; 8];
    unsafe {
        libc::arc4random_buf(b.as_mut_ptr() as *mut _, 8);
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("open db: {e}"))?;
        conn.execute_batch(
            "
PRAGMA journal_mode=WAL;
CREATE TABLE IF NOT EXISTS personas(
  id TEXT PRIMARY KEY, engine TEXT NOT NULL, name TEXT NOT NULL,
  color TEXT NOT NULL, bound INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS rooms(
  id TEXT PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL,
  members TEXT NOT NULL, cwd TEXT NOT NULL, permission TEXT NOT NULL DEFAULT 'project',
  default_target TEXT NOT NULL DEFAULT 'last', max_auto_turns INTEGER NOT NULL DEFAULT 6,
  dm_settings TEXT, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS messages(
  id INTEGER PRIMARY KEY AUTOINCREMENT, room_id TEXT NOT NULL, sender TEXT NOT NULL,
  role TEXT NOT NULL, text TEXT NOT NULL, run_id TEXT, reply_to INTEGER, created_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS idx_messages_room ON messages(room_id, id);
CREATE TABLE IF NOT EXISTS runs(
  id TEXT PRIMARY KEY, room_id TEXT, persona_id TEXT, task_json TEXT NOT NULL,
  status TEXT, session_id TEXT, tokens INTEGER, duration_s REAL, synthesized INTEGER DEFAULT 0,
  answer_text TEXT, created_at TEXT NOT NULL, finished_at TEXT);
CREATE INDEX IF NOT EXISTS idx_runs_room ON runs(room_id, persona_id, finished_at);
CREATE TABLE IF NOT EXISTS run_nodes(
  id TEXT PRIMARY KEY, run_id TEXT NOT NULL, node_seq INTEGER, persona_id TEXT,
  depends_on TEXT, status TEXT, winner INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS scores(
  id INTEGER PRIMARY KEY AUTOINCREMENT, persona_id TEXT NOT NULL, room_id TEXT,
  event TEXT NOT NULL, delta INTEGER NOT NULL, detail TEXT, created_at TEXT NOT NULL);
",
        )
        .map_err(|e| format!("migrate: {e}"))?;
        Ok(Self(conn))
    }

    /// 绑定向导：为勾选的 engine 各建一个 persona（幂等：已绑定跳过）
    pub fn bind_personas(&self, engines: &[String]) -> Result<Value, String> {
        let mut out = Vec::new();
        for engine in engines {
            let existing: Option<String> = self
                .0
                .query_row(
                    "SELECT id FROM personas WHERE engine=?1 ORDER BY created_at LIMIT 1",
                    params![engine],
                    |r| r.get(0),
                )
                .ok();
            if let Some(id) = existing {
                out.push(self.get_persona(&id)?);
                continue;
            }
            let id = format!("p-{}", uuid8());
            self.0
                .execute(
                    "INSERT INTO personas(id,engine,name,color,bound,created_at) VALUES(?1,?2,?3,?4,1,?5)",
                    params![id, engine, engine_label(engine), engine_color(engine), now()],
                )
                .map_err(|e| e.to_string())?;
            out.push(self.get_persona(&id)?);
        }
        Ok(Value::Array(out))
    }

    pub fn get_persona(&self, id: &str) -> Result<Value, String> {
        self.0
            .query_row(
                "SELECT id,engine,name,color,bound FROM personas WHERE id=?1",
                params![id],
                |r| {
                    Ok(json!({
                        "id": r.get::<_,String>(0)?, "engine": r.get::<_,String>(1)?,
                        "name": r.get::<_,String>(2)?, "color": r.get::<_,String>(3)?,
                        "bound": r.get::<_,i64>(4)? == 1,
                    }))
                },
            )
            .map_err(|e| e.to_string())
    }

    pub fn list_personas(&self) -> Result<Value, String> {
        let mut st = self
            .0
            .prepare("SELECT id FROM personas ORDER BY created_at")
            .map_err(|e| e.to_string())?;
        let ids: Vec<String> = st
            .query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        Ok(Value::Array(
            ids.iter().map(|id| self.get_persona(id)).collect::<Result<_, _>>()?,
        ))
    }

    /// 私聊：get-or-create 隐式房间（name=对方名，cwd=~/AgentChatProjects/<名>）
    pub fn open_dm(&self, persona_id: &str, projects_root: &Path) -> Result<Value, String> {
        if let Some(id) = self
            .0
            .query_row(
                "SELECT id FROM rooms WHERE kind='dm' AND members=?1",
                params![format!("[\"{persona_id}\"]")],
                |r| r.get::<_, String>(0),
            )
            .ok()
        {
            return self.get_room(&id);
        }
        let persona = self.get_persona(persona_id)?;
        let name = persona["name"].as_str().unwrap_or("dm").to_string();
        let safe: String = name
            .chars()
            .map(|c| if "/\\:*?\"<>| ".contains(c) { '-' } else { c })
            .collect();
        let cwd = projects_root.join(safe);
        std::fs::create_dir_all(&cwd).map_err(|e| format!("mkdir room cwd: {e}"))?;
        let id = format!("r-{}", uuid8());
        self.0
            .execute(
                "INSERT INTO rooms(id,kind,name,members,cwd,permission,default_target,max_auto_turns,created_at)
                 VALUES(?1,'dm',?2,?3,?4,'project','last',6,?5)",
                params![
                    id,
                    name,
                    format!("[\"{persona_id}\"]"),
                    cwd.display().to_string(),
                    now()
                ],
            )
            .map_err(|e| e.to_string())?;
        self.get_room(&id)
    }

    pub fn get_room(&self, id: &str) -> Result<Value, String> {
        self.0
            .query_row(
                "SELECT id,kind,name,members,cwd,permission,default_target,max_auto_turns FROM rooms WHERE id=?1",
                params![id],
                |r| {
                    Ok(json!({
                        "id": r.get::<_,String>(0)?, "kind": r.get::<_,String>(1)?,
                        "name": r.get::<_,String>(2)?,
                        "members": serde_json::from_str::<Value>(&r.get::<_,String>(3)?).unwrap_or(json!([])),
                        "cwd": r.get::<_,String>(4)?, "permission": r.get::<_,String>(5)?,
                        "default_target": r.get::<_,String>(6)?, "max_auto_turns": r.get::<_,i64>(7)?,
                    }))
                },
            )
            .map_err(|e| e.to_string())
    }

    /// 会话列表：房间 + 最后一条消息摘要
    pub fn list_rooms(&self) -> Result<Value, String> {
        let mut st = self
            .0
            .prepare(
                "SELECT r.id,
                   (SELECT text FROM messages WHERE room_id=r.id ORDER BY id DESC LIMIT 1),
                   (SELECT created_at FROM messages WHERE room_id=r.id ORDER BY id DESC LIMIT 1),
                   (SELECT sender FROM messages WHERE room_id=r.id ORDER BY id DESC LIMIT 1)
             FROM rooms r ORDER BY COALESCE((SELECT id FROM messages WHERE room_id=r.id ORDER BY id DESC LIMIT 1),0) DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<(String, Option<String>, Option<String>, Option<String>)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        let mut out = Vec::new();
        for (id, last, at, sender) in rows {
            let mut room = self.get_room(&id)?;
            room["last_message"] = last.map(Value::from).unwrap_or(Value::Null);
            room["last_at"] = at.map(Value::from).unwrap_or(Value::Null);
            room["last_sender"] = sender.map(Value::from).unwrap_or(Value::Null);
            out.push(room);
        }
        Ok(Value::Array(out))
    }

    pub fn add_message(
        &self,
        room_id: &str,
        sender: &str,
        role: &str,
        text: &str,
        run_id: Option<&str>,
    ) -> Result<i64, String> {
        self.0
            .execute(
                "INSERT INTO messages(room_id,sender,role,text,run_id,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
                params![room_id, sender, role, text, run_id, now()],
            )
            .map_err(|e| e.to_string())?;
        Ok(self.0.last_insert_rowid())
    }

    pub fn room_history(&self, room_id: &str, limit: i64) -> Result<Value, String> {
        let mut st = self
            .0
            .prepare(
                "SELECT m.id,m.sender,m.role,m.text,m.run_id,m.created_at,
                        r.status,r.tokens,r.duration_s,r.session_id
                 FROM messages m LEFT JOIN runs r ON m.run_id=r.id
                 WHERE m.room_id=?1 ORDER BY m.id DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<Value> = st
            .query_map(params![room_id, limit], |r| {
                Ok(json!({
                    "id": r.get::<_,i64>(0)?, "sender": r.get::<_,String>(1)?,
                    "role": r.get::<_,String>(2)?, "text": r.get::<_,String>(3)?,
                    "run_id": r.get::<_,Option<String>>(4)?, "created_at": r.get::<_,String>(5)?,
                    "status": r.get::<_,Option<String>>(6)?, "tokens": r.get::<_,Option<i64>>(7)?,
                    "duration_s": r.get::<_,Option<f64>>(8)?, "session_id": r.get::<_,Option<String>>(9)?,
                }))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        Ok(Value::Array(rows.into_iter().rev().collect()))
    }

    /// 续聊：房间内该 persona 最近一次成功的 session_id（§4.4 完成协议）
    pub fn last_session(&self, room_id: &str, persona_id: &str) -> Option<String> {
        self.0
            .query_row(
                "SELECT session_id FROM runs WHERE room_id=?1 AND persona_id=?2
                 AND session_id IS NOT NULL AND status='success'
                 ORDER BY finished_at DESC, rowid DESC LIMIT 1",
                params![room_id, persona_id],
                |r| r.get(0),
            )
            .ok()
    }

    pub fn add_run(&self, run: &Value, room_id: &str, persona_id: &str, task_json: &str) -> Result<(), String> {
        self.0
            .execute(
                "INSERT OR REPLACE INTO runs(id,room_id,persona_id,task_json,status,session_id,tokens,duration_s,synthesized,answer_text,created_at,finished_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![
                    run["task_id"].as_str().unwrap_or(""),
                    room_id,
                    persona_id,
                    task_json,
                    run["status"].as_str().unwrap_or("failed"),
                    run["session_id"].as_str(),
                    run["tokens"].as_i64(),
                    run["duration_s"].as_f64(),
                    if run["synthesized"].as_bool().unwrap_or(false) { 1 } else { 0 },
                    run["answer_text"].as_str().unwrap_or(""),
                    run["started_at"].as_str().map(String::from).unwrap_or_else(now),
                    run["finished_at"].as_str().map(String::from).unwrap_or_else(now),
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
