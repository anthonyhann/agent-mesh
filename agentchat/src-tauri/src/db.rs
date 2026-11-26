//! AgentChat SQLite 层（§4.3 会话事实源）
//! DB 位于 MESH_DATA 顶层 agentchat.db；results/ 仅为不可变 artifact 仓库。
//! Schema 版本化（§4.10）：PRAGMA user_version；v0=M1 旧库 → v1=v2.4 schema。

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::Path;

pub struct Db(pub Connection);

/// 当前 schema 版本（§4.10：v0=M1 旧库 → v1=v2.4）
pub const SCHEMA_VERSION: i64 = 1;

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

/// ULID（§4.11：client_msg_id = msg-<ulid>）：48bit 毫秒时间 + 80bit 随机，Crockford base32
pub fn ulid() -> String {
    const CROCK: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut tv: libc::timeval = unsafe { std::mem::zeroed() };
    unsafe { libc::gettimeofday(&mut tv, std::ptr::null_mut()) };
    let ms = (tv.tv_sec as u128) * 1000 + (tv.tv_usec as u128) / 1000;
    let mut rand = [0u8; 10];
    unsafe {
        libc::arc4random_buf(rand.as_mut_ptr() as *mut _, 10);
    }
    let lo: u128 = rand.iter().fold(0u128, |a, &b| (a << 8) | b as u128);
    let v: u128 = ((ms & 0xFFFF_FFFF_FFFF) << 80) | lo;
    (0..26)
        .map(|i| CROCK[((v >> (125 - 5 * i)) & 31) as usize] as char)
        .collect()
}

/// v1 全量 schema（新库直接建；老库经迁移殊途同归）
const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS personas(
  id TEXT PRIMARY KEY, engine TEXT NOT NULL, name TEXT NOT NULL,
  color TEXT NOT NULL, bound INTEGER NOT NULL DEFAULT 1, behavior_md TEXT,
  created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS rooms(
  id TEXT PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL,
  members TEXT NOT NULL, cwd TEXT NOT NULL, permission TEXT NOT NULL DEFAULT 'project',
  default_target TEXT NOT NULL DEFAULT 'last', max_auto_turns INTEGER NOT NULL DEFAULT 6,
  dm_settings TEXT,
  reply_mode TEXT NOT NULL DEFAULT 'all', default_target_persona_id TEXT,
  owner_persona_id TEXT, batch_delay_s INTEGER NOT NULL DEFAULT 10,
  auto_turn_limit INTEGER NOT NULL DEFAULT 3, group_prompt TEXT,
  chain_react INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS room_members(
  room_id TEXT NOT NULL, persona_id TEXT NOT NULL,
  muted INTEGER NOT NULL DEFAULT 0, reply_mode TEXT,
  PRIMARY KEY(room_id, persona_id));
CREATE TABLE IF NOT EXISTS messages(
  id INTEGER PRIMARY KEY AUTOINCREMENT, room_id TEXT NOT NULL, sender TEXT NOT NULL,
  role TEXT NOT NULL, text TEXT NOT NULL, turn_id TEXT, reply_to INTEGER,
  client_msg_id TEXT, recalled INTEGER NOT NULL DEFAULT 0, legacy_run_id TEXT,
  created_at TEXT NOT NULL);
CREATE UNIQUE INDEX IF NOT EXISTS uq_messages_client ON messages(room_id, client_msg_id);
CREATE INDEX IF NOT EXISTS idx_messages_room ON messages(room_id, id);
CREATE TABLE IF NOT EXISTS turns(
  turn_id TEXT PRIMARY KEY, room_id TEXT NOT NULL, trigger_msg_ids TEXT NOT NULL,
  target_personas TEXT NOT NULL, status TEXT NOT NULL, lane_seq INTEGER,
  claimed_at TEXT, lease_until TEXT, attempt INTEGER NOT NULL DEFAULT 1,
  parent_turn_id TEXT, created_at TEXT NOT NULL, completed_at TEXT,
  cancel_reason TEXT, last_error TEXT);
CREATE INDEX IF NOT EXISTS idx_turns_room ON turns(room_id, lane_seq);
CREATE TABLE IF NOT EXISTS runs(
  id TEXT PRIMARY KEY, room_id TEXT, persona_id TEXT, task_json TEXT NOT NULL,
  status TEXT, session_id TEXT, tokens INTEGER, duration_s REAL, synthesized INTEGER DEFAULT 0,
  answer_text TEXT, turn_id TEXT, attempt INTEGER NOT NULL DEFAULT 1, grants TEXT,
  created_at TEXT NOT NULL, finished_at TEXT);
CREATE UNIQUE INDEX IF NOT EXISTS uq_runs_turn ON runs(turn_id, persona_id, attempt) WHERE turn_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_runs_room ON runs(room_id, persona_id, finished_at);
CREATE TABLE IF NOT EXISTS room_persona_sessions(
  room_id TEXT NOT NULL, persona_id TEXT NOT NULL,
  session_id TEXT, sandbox TEXT, status TEXT, archived TEXT,
  created_at TEXT NOT NULL,
  PRIMARY KEY(room_id, persona_id));
CREATE TABLE IF NOT EXISTS room_summaries(
  room_id TEXT PRIMARY KEY, summary_text TEXT, summary_through_msg_id INTEGER,
  summary_tokens INTEGER, model TEXT, prompt_version TEXT, summary_error TEXT,
  version INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS approvals(
  id INTEGER PRIMARY KEY AUTOINCREMENT, scope TEXT, tool_name TEXT, args_hash TEXT,
  grant_expires_at TEXT, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS run_nodes(
  id TEXT PRIMARY KEY, run_id TEXT NOT NULL, node_seq INTEGER, persona_id TEXT,
  depends_on TEXT, status TEXT, winner INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS scores(
  id INTEGER PRIMARY KEY AUTOINCREMENT, persona_id TEXT NOT NULL, room_id TEXT,
  event TEXT NOT NULL, delta INTEGER NOT NULL, detail TEXT, created_at TEXT NOT NULL);
";

impl Db {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("mkdir db dir: {e}"))?;
        }
        let conn = Connection::open(path).map_err(|e| format!("open db: {e}"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| format!("wal: {e}"))?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| format!("user_version: {e}"))?;
        match version.cmp(&SCHEMA_VERSION) {
            std::cmp::Ordering::Greater => {
                return Err(format!(
                    "db schema v{version} 比 app 支持的 v{SCHEMA_VERSION} 新，请升级 AgentChat"
                ))
            }
            std::cmp::Ordering::Equal => {
                // 幂等兜底：老版本 app 建的 v1 可能缺后续幂等对象
                conn.execute_batch(SCHEMA_V1).map_err(|e| format!("schema v1: {e}"))?;
            }
            std::cmp::Ordering::Less => {
                let fresh: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                        [],
                        |r| r.get::<_, i64>(0),
                    )
                    .map_err(|e| e.to_string())?
                    == 0;
                if fresh {
                    conn.execute_batch(SCHEMA_V1).map_err(|e| format!("schema v1: {e}"))?;
                    conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
                        .map_err(|e| format!("user_version set: {e}"))?;
                } else {
                    // v0（M1 旧库）→ v1：先备份再迁移（§4.10 定型顺序）
                    let db = Self(conn);
                    let backup = db.backup_for_migration(path)?;
                    if let Err(e) = db.migrate_v0_to_v1() {
                        return Err(format!(
                            "schema 迁移失败（已回滚，备份在 {}）：{e}",
                            backup.display()
                        ));
                    }
                    return Ok(db);
                }
            }
        }
        Ok(Self(conn))
    }

    /// 迁移前备份：VACUUM INTO（事务外，WAL 安全）→ backups/ 0700，只留最近 5 份
    fn backup_for_migration(&self, db_path: &Path) -> Result<std::path::PathBuf, String> {
        let dir = db_path
            .parent()
            .ok_or("db path has no parent")?
            .join("backups");
        std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir backups: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("chmod backups: {e}"))?;
        }
        let stamp = now().replace(['-', ':'], "");
        let mut dest = None;
        for _ in 0..10 {
            // 预检不存在，生成带时间戳的唯一备份文件名
            let candidate = dir.join(format!("agentchat-{stamp}-{}.db", &uuid8()[..4]));
            if !candidate.exists() {
                dest = Some(candidate);
                break;
            }
        }
        let dest = dest.ok_or("无法生成唯一备份文件名")?;
        let escaped = dest.display().to_string().replace('\'', "''");
        self.0
            .execute_batch(&format!("VACUUM INTO '{escaped}'"))
            .map_err(|e| format!("VACUUM INTO: {e}"))?;
        // 只保留最近 5 份
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok().map(|x| x.path()))
            .filter(|p| p.extension().map(|x| x == "db").unwrap_or(false))
            .collect();
        files.sort();
        while files.len() > 5 {
            let victim = files.remove(0);
            let _ = std::fs::remove_file(victim);
        }
        Ok(dest)
    }

    /// v0 → v1 迁移事务：任何一步失败整体回滚（备份已在事务外完成）
    fn migrate_v0_to_v1(&self) -> Result<(), String> {
        let tx = self.0.unchecked_transaction().map_err(|e| e.to_string())?;

        // —— personas：+behavior_md ——
        add_column_if_missing(&tx, "personas", "behavior_md", "behavior_md TEXT")?;

        // —— rooms：+v2.4 房间字段 ——
        for (col, ddl) in [
            ("reply_mode", "reply_mode TEXT NOT NULL DEFAULT 'all'"),
            ("default_target_persona_id", "default_target_persona_id TEXT"),
            ("owner_persona_id", "owner_persona_id TEXT"),
            ("batch_delay_s", "batch_delay_s INTEGER NOT NULL DEFAULT 10"),
            ("auto_turn_limit", "auto_turn_limit INTEGER NOT NULL DEFAULT 3"),
            ("group_prompt", "group_prompt TEXT"),
            ("chain_react", "chain_react INTEGER NOT NULL DEFAULT 0"),
        ] {
            add_column_if_missing(&tx, "rooms", col, ddl)?;
        }
        // 轮数闸门合一：旧 max_auto_turns 值迁入 auto_turn_limit
        tx.execute(
            "UPDATE rooms SET auto_turn_limit=max_auto_turns WHERE auto_turn_limit=3 AND max_auto_turns<>6",
            [],
        )
        .map_err(|e| format!("rooms backfill auto_turn_limit: {e}"))?;

        // —— room_members：members JSON → 表行；owner_persona_id=首个成员 ——
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS room_members(
              room_id TEXT NOT NULL, persona_id TEXT NOT NULL,
              muted INTEGER NOT NULL DEFAULT 0, reply_mode TEXT,
              PRIMARY KEY(room_id, persona_id));",
        )
        .map_err(|e| format!("create room_members: {e}"))?;
        {
            let mut st = tx
                .prepare("SELECT id, members FROM rooms")
                .map_err(|e| e.to_string())?;
            let rows: Vec<(String, String)> = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(|e| e.to_string())?
                .filter_map(|x| x.ok())
                .collect();
            drop(st);
            for (room_id, members_json) in rows {
                let members: Vec<String> = serde_json::from_str(&members_json).unwrap_or_default();
                for (i, pid) in members.iter().enumerate() {
                    tx.execute(
                        "INSERT OR IGNORE INTO room_members(room_id,persona_id) VALUES(?1,?2)",
                        params![room_id, pid],
                    )
                    .map_err(|e| format!("room_members backfill: {e}"))?;
                    if i == 0 {
                        tx.execute(
                            "UPDATE rooms SET owner_persona_id=?2 WHERE id=?1 AND owner_persona_id IS NULL",
                            params![room_id, pid],
                        )
                        .map_err(|e| format!("owner_persona_id backfill: {e}"))?;
                    }
                }
            }
        }

        // —— 测试钩子：注入迁移中途失败（验收回滚+备份恢复路径）——
        if std::env::var("AGENTCHAT_MIGRATE_FAIL_AFTER").as_deref() == Ok("rooms") {
            return Err("injected migration failure after rooms".into());
        }

        // —— messages 重建：保留原 id、backfill client_msg_id、run_id→legacy_run_id ——
        tx.execute_batch(
            "CREATE TABLE messages_new(
              id INTEGER PRIMARY KEY AUTOINCREMENT, room_id TEXT NOT NULL, sender TEXT NOT NULL,
              role TEXT NOT NULL, text TEXT NOT NULL, turn_id TEXT, reply_to INTEGER,
              client_msg_id TEXT, recalled INTEGER NOT NULL DEFAULT 0, legacy_run_id TEXT,
              created_at TEXT NOT NULL);
            INSERT INTO messages_new(id,room_id,sender,role,text,turn_id,reply_to,client_msg_id,recalled,legacy_run_id,created_at)
              SELECT id,room_id,sender,role,text,NULL,reply_to,'msg-legacy-'||id,0,run_id,created_at FROM messages;
            DROP TABLE messages;
            ALTER TABLE messages_new RENAME TO messages;
            CREATE UNIQUE INDEX IF NOT EXISTS uq_messages_client ON messages(room_id, client_msg_id);
            CREATE INDEX IF NOT EXISTS idx_messages_room ON messages(room_id, id);
            UPDATE sqlite_sequence SET seq=(SELECT COALESCE(MAX(id),0) FROM messages) WHERE name='messages';",
        )
        .map_err(|e| format!("messages rebuild: {e}"))?;

        // —— turns：房间回合持久化（M2a-2 启用，先建表）——
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS turns(
              turn_id TEXT PRIMARY KEY, room_id TEXT NOT NULL, trigger_msg_ids TEXT NOT NULL,
              target_personas TEXT NOT NULL, status TEXT NOT NULL, lane_seq INTEGER,
              claimed_at TEXT, lease_until TEXT, attempt INTEGER NOT NULL DEFAULT 1,
              parent_turn_id TEXT, created_at TEXT NOT NULL, completed_at TEXT,
              cancel_reason TEXT, last_error TEXT);
            CREATE INDEX IF NOT EXISTS idx_turns_room ON turns(room_id, lane_seq);",
        )
        .map_err(|e| format!("create turns: {e}"))?;

        // —— runs：+turn_id/attempt/grants 快照 + 部分唯一索引 ——
        for (col, ddl) in [
            ("turn_id", "turn_id TEXT"),
            ("attempt", "attempt INTEGER NOT NULL DEFAULT 1"),
            ("grants", "grants TEXT"),
        ] {
            add_column_if_missing(&tx, "runs", col, ddl)?;
        }
        tx.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_runs_turn ON runs(turn_id, persona_id, attempt) WHERE turn_id IS NOT NULL;",
        )
        .map_err(|e| format!("runs unique index: {e}"))?;

        // —— room_persona_sessions：从最近成功 run backfill（K3 权威表）——
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS room_persona_sessions(
              room_id TEXT NOT NULL, persona_id TEXT NOT NULL,
              session_id TEXT, sandbox TEXT, status TEXT, archived TEXT,
              created_at TEXT NOT NULL,
              PRIMARY KEY(room_id, persona_id));",
        )
        .map_err(|e| format!("create room_persona_sessions: {e}"))?;
        tx.execute(
            "INSERT OR REPLACE INTO room_persona_sessions(room_id,persona_id,session_id,status,created_at)
             SELECT room_id, persona_id, session_id, 'active', MAX(finished_at)
             FROM runs
             WHERE session_id IS NOT NULL AND status='success' AND room_id IS NOT NULL AND persona_id IS NOT NULL
             GROUP BY room_id, persona_id",
            [],
        )
        .map_err(|e| format!("room_persona_sessions backfill: {e}"))?;

        // —— room_summaries / approvals（P2 预留）——
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS room_summaries(
              room_id TEXT PRIMARY KEY, summary_text TEXT, summary_through_msg_id INTEGER,
              summary_tokens INTEGER, model TEXT, prompt_version TEXT, summary_error TEXT,
              version INTEGER NOT NULL DEFAULT 0);
            CREATE TABLE IF NOT EXISTS approvals(
              id INTEGER PRIMARY KEY AUTOINCREMENT, scope TEXT, tool_name TEXT, args_hash TEXT,
              grant_expires_at TEXT, created_at TEXT NOT NULL);",
        )
        .map_err(|e| format!("create summaries/approvals: {e}"))?;

        tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
            .map_err(|e| format!("user_version set: {e}"))?;
        tx.commit().map_err(|e| format!("migrate commit: {e}"))?;
        Ok(())
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
                "SELECT room_id FROM room_members WHERE persona_id=?1 AND room_id IN (SELECT id FROM rooms WHERE kind='dm') LIMIT 1",
                params![persona_id],
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
                "INSERT INTO rooms(id,kind,name,members,cwd,permission,default_target,max_auto_turns,owner_persona_id,created_at)
                 VALUES(?1,'dm',?2,?3,?4,'project','last',6,?5,?6)",
                params![
                    id,
                    name,
                    format!("[\"{persona_id}\"]"),
                    cwd.display().to_string(),
                    persona_id,
                    now()
                ],
            )
            .map_err(|e| e.to_string())?;
        self.0
            .execute(
                "INSERT OR IGNORE INTO room_members(room_id,persona_id) VALUES(?1,?2)",
                params![id, persona_id],
            )
            .map_err(|e| e.to_string())?;
        self.get_room(&id)
    }

    pub fn get_room(&self, id: &str) -> Result<Value, String> {
        self.0
            .query_row(
                "SELECT id,kind,name,cwd,permission,reply_mode,default_target_persona_id,owner_persona_id,
                        batch_delay_s,auto_turn_limit,chain_react
                 FROM rooms WHERE id=?1",
                params![id],
                |r| {
                    Ok(json!({
                        "id": r.get::<_,String>(0)?, "kind": r.get::<_,String>(1)?,
                        "name": r.get::<_,String>(2)?,
                        "cwd": r.get::<_,String>(3)?, "permission": r.get::<_,String>(4)?,
                        "reply_mode": r.get::<_,String>(5)?,
                        "default_target_persona_id": r.get::<_,Option<String>>(6)?,
                        "owner_persona_id": r.get::<_,Option<String>>(7)?,
                        "batch_delay_s": r.get::<_,i64>(8)?, "auto_turn_limit": r.get::<_,i64>(9)?,
                        "chain_react": r.get::<_,i64>(10)? == 1,
                    }))
                },
            )
            .and_then(|mut room| {
                // members 权威源 = room_members 表（迁移已 backfill）
                let mut st = self.0.prepare(
                    "SELECT persona_id FROM room_members WHERE room_id=?1 ORDER BY rowid",
                )?;
                let members: Vec<String> = st
                    .query_map(params![id], |r| r.get(0))?
                    .filter_map(|x| x.ok())
                    .collect();
                if members.is_empty() {
                    // 极端兜底：老库未迁移（不应发生）
                    let legacy: String = self.0.query_row(
                        "SELECT members FROM rooms WHERE id=?1",
                        params![id],
                        |r| r.get(0),
                    )?;
                    room["members"] = serde_json::from_str(&legacy).unwrap_or(json!([]));
                } else {
                    room["members"] = json!(members);
                }
                Ok(room)
            })
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

    /// 写入消息：client_msg_id 可由调用方（前端）传入以支持重试幂等（§4.11），
    /// 未传则由本层自铸 msg-<ulid>。UNIQUE(room_id, client_msg_id) 幂等——重复提交返回既有消息 id。
    /// turn_id / legacy_run_id 二选一携带（M2a-2 前气泡走 legacy_run_id 关联）。
    pub fn add_message(
        &self,
        room_id: &str,
        sender: &str,
        role: &str,
        text: &str,
        turn_id: Option<&str>,
        legacy_run_id: Option<&str>,
        client_msg_id: Option<&str>,
    ) -> Result<(i64, String), String> {
        let client_msg_id = client_msg_id.map(str::to_string).unwrap_or_else(|| format!("msg-{}", ulid()));
        self.0
            .execute(
                "INSERT INTO messages(room_id,sender,role,text,turn_id,client_msg_id,legacy_run_id,created_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
                 ON CONFLICT(room_id, client_msg_id) DO NOTHING",
                params![room_id, sender, role, text, turn_id, client_msg_id, legacy_run_id, now()],
            )
            .map_err(|e| e.to_string())?;
        let id: i64 = self
            .0
            .query_row(
                "SELECT id FROM messages WHERE room_id=?1 AND client_msg_id=?2",
                params![room_id, client_msg_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok((id, client_msg_id))
    }

    /// 历史（§4.3 join 规则）：persona 消息按 (turn_id, sender) 取最新 attempt；
    /// turn_id 为空（迁移旧数据 / M2a-2 前过渡期）回退 legacy_run_id；user 消息不 join。
    pub fn room_history(&self, room_id: &str, limit: i64) -> Result<Value, String> {
        let mut st = self
            .0
            .prepare(
                "SELECT m.id,m.sender,m.role,m.text,m.turn_id,m.legacy_run_id,m.client_msg_id,m.created_at,
                        r.status,r.tokens,r.duration_s,r.session_id
                 FROM messages m
                 LEFT JOIN runs r ON (
                   (m.turn_id IS NOT NULL AND r.turn_id=m.turn_id AND r.persona_id=m.sender
                     AND r.attempt=(SELECT MAX(attempt) FROM runs WHERE turn_id=m.turn_id AND persona_id=m.sender))
                   OR (m.turn_id IS NULL AND m.legacy_run_id IS NOT NULL AND r.id=m.legacy_run_id)
                 )
                 WHERE m.room_id=?1 ORDER BY m.id DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<Value> = st
            .query_map(params![room_id, limit], |r| {
                Ok(json!({
                    "id": r.get::<_,i64>(0)?, "sender": r.get::<_,String>(1)?,
                    "role": r.get::<_,String>(2)?, "text": r.get::<_,String>(3)?,
                    "turn_id": r.get::<_,Option<String>>(4)?,
                    "run_id": r.get::<_,Option<String>>(5)?,
                    "client_msg_id": r.get::<_,Option<String>>(6)?, "created_at": r.get::<_,String>(7)?,
                    "status": r.get::<_,Option<String>>(8)?, "tokens": r.get::<_,Option<i64>>(9)?,
                    "duration_s": r.get::<_,Option<f64>>(10)?, "session_id": r.get::<_,Option<String>>(11)?,
                }))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        Ok(Value::Array(rows.into_iter().rev().collect()))
    }

    /// 续聊（§4.3 K3：权威表 room_persona_sessions，不回读 runs.task_json）
    pub fn last_session(&self, room_id: &str, persona_id: &str) -> Option<String> {
        self.0
            .query_row(
                "SELECT session_id FROM room_persona_sessions
                 WHERE room_id=?1 AND persona_id=?2 AND session_id IS NOT NULL AND status='active'",
                params![room_id, persona_id],
                |r| r.get(0),
            )
            .ok()
    }

    /// 落 run（§4.3：UNIQUE(turn_id, persona_id, attempt)；M2a-2 前 turn_id 可空走 INSERT OR REPLACE by id）。
    /// 成功且带 session_id 时维护 room_persona_sessions：换 session 旧值进 archived JSON（可审计）。
    pub fn add_run(
        &self,
        run: &Value,
        room_id: &str,
        persona_id: &str,
        task_json: &str,
        turn_id: Option<&str>,
        attempt: i64,
    ) -> Result<(), String> {
        self.0
            .execute(
                "INSERT OR REPLACE INTO runs(id,room_id,persona_id,task_json,status,session_id,tokens,duration_s,synthesized,answer_text,turn_id,attempt,created_at,finished_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
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
                    turn_id,
                    attempt,
                    run["started_at"].as_str().map(String::from).unwrap_or_else(now),
                    run["finished_at"].as_str().map(String::from).unwrap_or_else(now),
                ],
            )
            .map_err(|e| e.to_string())?;
        if run["status"].as_str() == Some("success") {
            if let Some(session_id) = run["session_id"].as_str() {
                upsert_session(&self.0, room_id, persona_id, session_id)?;
            }
        }
        Ok(())
    }

    // ---------- M2a-2：房间 lane（turns 持久化 + 完成事务 + 崩溃恢复） ----------

    /// §6.2.1 消息+turn 同事务：用户消息落库与 turn 创建不可分割。
    /// 幂等（§4.11 第一层）：重复 client_msg_id → 同一条消息、同一个 turn（created=false），
    /// 由调用方决定不再消费（绝不双跑）。返回 (msg_id, client_msg_id, turn_id, created)。
    pub fn add_user_message_turn(
        &self,
        room_id: &str,
        text: &str,
        client_msg_id: Option<&str>,
        target_personas: &[String],
    ) -> Result<(i64, String, String, bool), String> {
        let cmid = client_msg_id
            .map(str::to_string)
            .unwrap_or_else(|| format!("msg-{}", ulid()));
        let tx = self.0.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO messages(room_id,sender,role,text,client_msg_id,created_at)
             VALUES(?1,'user','user',?2,?3,?4)
             ON CONFLICT(room_id, client_msg_id) DO NOTHING",
            params![room_id, text, cmid, now()],
        )
        .map_err(|e| e.to_string())?;
        let msg_id: i64 = tx
            .query_row(
                "SELECT id FROM messages WHERE room_id=?1 AND client_msg_id=?2",
                params![room_id, cmid],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        // 该消息已有 turn？（重试场景：同 client_msg_id → 同一 turn，绝不双消费）
        let trig = format!("[{msg_id}]");
        let existing: Option<String> = tx
            .query_row(
                "SELECT turn_id FROM turns WHERE room_id=?1 AND trigger_msg_ids=?2",
                params![room_id, trig],
                |r| r.get(0),
            )
            .ok();
        if let Some(tid) = existing {
            tx.commit().map_err(|e| e.to_string())?;
            return Ok((msg_id, cmid, tid, false));
        }
        let turn_id = format!("turn-{}", ulid());
        let lane_seq: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(lane_seq),0)+1 FROM turns WHERE room_id=?1",
                params![room_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let targets =
            serde_json::to_string(target_personas).unwrap_or_else(|_| "[]".into());
        tx.execute(
            "INSERT INTO turns(turn_id,room_id,trigger_msg_ids,target_personas,status,lane_seq,claimed_at,lease_until,attempt,created_at)
             VALUES(?1,?2,?3,?4,'claimed',?5,?6,?7,1,?6)",
            params![turn_id, room_id, trig, targets, lane_seq, now(), now_plus(LANE_LEASE_S)],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok((msg_id, cmid, turn_id, true))
    }

    /// §4.3 完成事务（codex BLOCKER）：run 落库 + persona 气泡 + turn 终态 = 单事务。
    /// 幂等：run 按 (turn_id,persona_id,attempt) ON CONFLICT DO NOTHING（§4.11 第三层）；
    /// 气泡按 (room_id,turn_id,sender) 查重——崩溃恢复重放不写第二条。
    /// turn 终态：本 run 失败 → failed；所有目标 persona 均有 success run → done；否则保持 claimed。
    pub fn complete_turn(
        &self,
        turn_id: &str,
        room_id: &str,
        persona_id: &str,
        run: &Value,
        task_json: &str,
        attempt: i64,
        bubble: &str,
    ) -> Result<(), String> {
        let tx = self.0.unchecked_transaction().map_err(|e| e.to_string())?;
        let run_id = run["task_id"].as_str().unwrap_or("").to_string();
        tx.execute(
            "INSERT INTO runs(id,room_id,persona_id,task_json,status,session_id,tokens,duration_s,synthesized,answer_text,turn_id,attempt,created_at,finished_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
             ON CONFLICT(turn_id,persona_id,attempt) WHERE turn_id IS NOT NULL DO NOTHING",
            params![
                run_id, room_id, persona_id, task_json,
                run["status"].as_str().unwrap_or("failed"),
                run["session_id"].as_str(),
                run["tokens"].as_i64(),
                run["duration_s"].as_f64(),
                if run["synthesized"].as_bool().unwrap_or(false) { 1 } else { 0 },
                run["answer_text"].as_str().unwrap_or(""),
                turn_id, attempt,
                run["started_at"].as_str().map(String::from).unwrap_or_else(now),
                run["finished_at"].as_str().map(String::from).unwrap_or_else(now),
            ],
        )
        .map_err(|e| e.to_string())?;
        let ok = run["status"].as_str() == Some("success");
        if ok {
            if let Some(sid) = run["session_id"].as_str() {
                upsert_session(&tx, room_id, persona_id, sid)?;
            }
        }
        // 气泡（幂等：同 turn 同 persona 只写一次）
        let has_bubble: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE room_id=?1 AND turn_id=?2 AND sender=?3",
                params![room_id, turn_id, persona_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if has_bubble == 0 {
            tx.execute(
                "INSERT INTO messages(room_id,sender,role,text,turn_id,client_msg_id,legacy_run_id,created_at)
                 VALUES(?1,?2,'persona',?3,?4,?5,?6,?7)",
                params![room_id, persona_id, bubble, turn_id, format!("msg-{}", ulid()), run_id, now()],
            )
            .map_err(|e| e.to_string())?;
        }
        // turn 终态：失败即 failed；全部目标 success 才 done（DM 单目标即时终态）
        let targets_json: String = tx
            .query_row(
                "SELECT target_personas FROM turns WHERE turn_id=?1",
                params![turn_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let targets: Vec<String> = serde_json::from_str(&targets_json).unwrap_or_default();
        let success_n: i64 = tx
            .query_row(
                "SELECT COUNT(DISTINCT persona_id) FROM runs WHERE turn_id=?1 AND status='success'",
                params![turn_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let (status, last_err): (&str, Option<String>) = if !ok {
            ("failed", Some(format!("{} run failed", persona_id)))
        } else if success_n >= targets.len() as i64 {
            ("done", None)
        } else {
            ("claimed", None)
        };
        if status != "claimed" {
            tx.execute(
                "UPDATE turns SET status=?2, completed_at=?3, attempt=?4, last_error=?5 WHERE turn_id=?1",
                params![turn_id, status, now(), attempt, last_err],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// §6.2.1 崩溃恢复扫描：非终态 turn（claimed/running）逐 persona 判定恢复动作。
    /// - 无结果的 running run → 先标 interrupted（§4.3 重试契约）；
    /// - run 成功但气泡缺失 → backfill（幂等补写，不重跑）；
    /// - run 缺失/失败/interrupted → rerun（attempt+1，重试=同 turn 新 attempt 新 run_id）。
    /// 只补未完成的 persona，不重跑已成功 run。返回动作清单供调用方执行。
    pub fn recover_scan(&self) -> Result<Vec<Value>, String> {
        let mut out: Vec<Value> = Vec::new();
        let mut st = self
            .0
            .prepare(
                "SELECT turn_id, room_id, target_personas, trigger_msg_ids, attempt FROM turns
                 WHERE status IN ('claimed','running') ORDER BY room_id, lane_seq",
            )
            .map_err(|e| e.to_string())?;
        let turns: Vec<(String, String, String, String, i64)> = st
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        drop(st);
        let mut busy_turns: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (turn_id, room_id, targets_json, trig_json, turn_attempt) in turns {
            let targets: Vec<String> = serde_json::from_str(&targets_json).unwrap_or_default();
            for persona_id in targets {
                // 无结果 running → interrupted
                self.0
                    .execute(
                        "UPDATE runs SET status='interrupted' WHERE turn_id=?1 AND persona_id=?2 AND status='running'",
                        params![turn_id, persona_id],
                    )
                    .map_err(|e| e.to_string())?;
                let latest: Option<(String, String, i64, String)> = self
                    .0
                    .query_row(
                        "SELECT id, COALESCE(status,'failed'), attempt, task_json FROM runs
                         WHERE turn_id=?1 AND persona_id=?2 ORDER BY attempt DESC LIMIT 1",
                        params![turn_id, persona_id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                    )
                    .ok();
                match latest {
                    None => {
                        busy_turns.insert(turn_id.clone());
                        // 无快照（崩溃在 run 创建前）：带触发消息原文供调用方重建任务卡
                        let brief = trigger_brief(&self.0, &trig_json);
                        out.push(json!({
                            "turn_id": turn_id, "room_id": room_id, "persona_id": persona_id,
                            "action": "rerun", "attempt": turn_attempt, "brief": brief,
                        }));
                    }
                    Some((rid, status, attempt, _task_json)) if status == "success" => {
                        let has_bubble: i64 = self
                            .0
                            .query_row(
                                "SELECT COUNT(*) FROM messages WHERE room_id=?1 AND turn_id=?2 AND sender=?3",
                                params![room_id, turn_id, persona_id],
                                |r| r.get(0),
                            )
                            .unwrap_or(0);
                        if has_bubble == 0 {
                            busy_turns.insert(turn_id.clone());
                            let ans: String = self
                                .0
                                .query_row(
                                    "SELECT COALESCE(answer_text,'') FROM runs WHERE id=?1",
                                    params![rid],
                                    |r| r.get(0),
                                )
                                .map_err(|e| e.to_string())?;
                            out.push(json!({
                                "turn_id": turn_id, "room_id": room_id, "persona_id": persona_id,
                                "action": "backfill", "attempt": attempt,
                                "run": {"task_id": rid, "status": "success", "answer_text": ans},
                            }));
                        }
                    }
                    Some((_rid, _status, attempt, task_json)) => {
                        busy_turns.insert(turn_id.clone());
                        out.push(json!({
                            "turn_id": turn_id, "room_id": room_id, "persona_id": persona_id,
                            "action": "rerun", "attempt": attempt + 1, "task_json": task_json,
                        }));
                    }
                }
            }
        }
        // 收尾 sweep：无待办动作的悬挂 turn（全部 persona 已 success+气泡）直接终态化
        let mut st2 = self
            .0
            .prepare("SELECT turn_id FROM turns WHERE status IN ('claimed','running')")
            .map_err(|e| e.to_string())?;
        let dangling: Vec<String> = st2
            .query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        drop(st2);
        for tid in dangling {
            if !busy_turns.contains(&tid) {
                self.0
                    .execute(
                        "UPDATE turns SET status='done', completed_at=?2 WHERE turn_id=?1",
                        params![tid, now()],
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(out)
    }
}

/// lane claim 租约秒数（覆盖内核 timeout_seconds=300 + 余量）
const LANE_LEASE_S: i64 = 330;

/// now() + n 秒（同 ISO8601 UTC 格式，供 lease_until 比较）
fn now_plus(secs: i64) -> String {
    unsafe {
        let t = libc::time(std::ptr::null_mut()) + secs;
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

/// room_persona_sessions 维护：换 session 旧值进 archived JSON（§4.3，不静默替换，可审计）。
/// 自由函数，供 add_run 与 complete_turn（事务内）共用。
fn upsert_session(
    conn: &Connection,
    room_id: &str,
    persona_id: &str,
    session_id: &str,
) -> Result<(), String> {
    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT session_id, COALESCE(archived,'[]') FROM room_persona_sessions
             WHERE room_id=?1 AND persona_id=?2",
            params![room_id, persona_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    match existing {
        Some((old, archived)) if old != session_id => {
            let mut arr: Vec<Value> = serde_json::from_str(&archived).unwrap_or_default();
            arr.push(json!({"session_id": old, "replaced_at": now()}));
            conn.execute(
                "UPDATE room_persona_sessions SET session_id=?3, archived=?4, status='active'
                 WHERE room_id=?1 AND persona_id=?2",
                params![
                    room_id,
                    persona_id,
                    session_id,
                    serde_json::to_string(&arr).unwrap_or_else(|_| "[]".into())
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        Some(_) => {}
        None => {
            conn.execute(
                "INSERT INTO room_persona_sessions(room_id,persona_id,session_id,status,created_at)
                 VALUES(?1,?2,?3,'active',?4)",
                params![room_id, persona_id, session_id, now()],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 取触发消息原文（trigger_msg_ids JSON 数组的第一条），供崩溃恢复重建任务卡
fn trigger_brief(conn: &Connection, trig_json: &str) -> String {
    let ids: Vec<i64> = serde_json::from_str(trig_json).unwrap_or_default();
    ids.first()
        .and_then(|id| {
            conn.query_row(
                "SELECT text FROM messages WHERE id=?1",
                params![id],
                |r| r.get::<_, String>(0),
            )
            .ok()
        })
        .unwrap_or_default()
}

/// ALTER TABLE ADD COLUMN（幂等：列已存在则跳过）
fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    ddl: &str,
) -> Result<(), String> {
    let exists: bool = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .filter_map(|x| x.ok())
        .any(|name| name == column);
    if !exists {
        conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {ddl}"))
            .map_err(|e| format!("alter {table} add {column}: {e}"))?;
    }
    Ok(())
}
