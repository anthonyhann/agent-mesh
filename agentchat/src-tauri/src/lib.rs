// AgentChat M0 后端：双根复制 · env doctor · sidecar spawn · watchdog(§4.8)
//
// 路径契约（design-agentchat.md §3）：
//   只读源  Contents/Resources/kernel/（dev 时回退 <manifest>/../kernel）
//   可写根  ~/Library/Application Support/AgentChat/kernel/<version>/（= MESH_ROOT）
//           kernel/current symlink 指向当前版本
// sidecar Contents/MacOS/mesh-run-<triple>（dev 时回退 target/debug/mesh-run）

use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::Manager;

mod db;
use db::Db;

const SIDECAR_TRIPLE: &str = "mesh-run-aarch64-apple-darwin";

// ---------- 路径 ----------

fn mesh_data() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join("Library/Application Support/AgentChat")
}

fn resource_kernel() -> Option<PathBuf> {
    // bundle：Contents/MacOS/agentchat → Contents/Resources/kernel
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent().and_then(|d| d.parent()).map(|c| c.join("Resources/kernel")) {
            if p.join("run_worker.sh").exists() {
                return Some(p);
            }
        }
    }
    // dev 回退：src-tauri/../kernel（tauri dev 时 resource_dir 指向 target/debug）
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kernel");
    if dev.join("run_worker.sh").exists() {
        return Some(dev);
    }
    None
}

fn sidecar_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("AGENTCHAT_MESH_RUN_BIN") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    for name in [SIDECAR_TRIPLE, "mesh-run"] {
        let p = dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

// ---------- 双根：首启复制 ----------

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_file() {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn ensure_kernel(app: Option<&tauri::AppHandle>) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    let version = env!("CARGO_PKG_VERSION");
    let target = mesh_data().join("kernel").join(version);

    // 只读源：resource_dir（打包）或 dev 回退
    let src = app
        .and_then(|a| a.path().resource_dir().ok())
        .map(|d| d.join("kernel"))
        .filter(|d| d.join("run_worker.sh").exists())
        .or_else(resource_kernel)
        .ok_or("kernel resources not found (neither bundle nor dev fallback)")?;

    if !target.join("run_worker.sh").exists() {
        let _ = fs::remove_dir_all(&target);
        copy_dir(&src, &target).map_err(|e| format!("copy kernel: {e}"))?;
        for entry in fs::read_dir(&target).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let mut perm = entry.metadata().map_err(|e| e.to_string())?.permissions();
            perm.set_mode(0o755);
            fs::set_permissions(entry.path(), perm).map_err(|e| e.to_string())?;
        }
    }
    // current symlink
    let current = mesh_data().join("kernel").join("current");
    let _ = fs::remove_file(&current);
    std::os::unix::fs::symlink(&target, &current).map_err(|e| format!("symlink current: {e}"))?;
    for d in ["tasks", "results"] {
        fs::create_dir_all(target.join(d)).map_err(|e| e.to_string())?;
    }
    Ok(current)
}

// ---------- env doctor ----------

fn login_path() -> String {
    Command::new("/bin/zsh")
        .args(["-lic", "printf %s \"$PATH\""])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::env::var("PATH").unwrap_or_default())
}

fn which_on(path: &str, tool: &str) -> Option<String> {
    for dir in path.split(':') {
        let p = Path::new(dir).join(tool);
        if p.is_file() {
            return Some(p.display().to_string());
        }
    }
    None
}

#[tauri::command]
fn doctor(state: tauri::State<'_, AppState>) -> Value {
    let root = state.mesh_root.lock().unwrap().clone();
    let path = state.login_path.lock().unwrap().clone();
    let tools = [
        "python3", "zstd", "timeout", "dsh", "codex", "opencode", "hermes", "arkcli", "claude",
        "qwen", "pi",
    ];
    let tool_map: Value = tools
        .iter()
        .map(|t| {
            (
                t.to_string(),
                which_on(&path, t).map(Value::from).unwrap_or(Value::Null),
            )
        })
        .collect::<serde_json::Map<String, Value>>()
        .into();
    let home = std::env::var("HOME").unwrap_or_default();
    let auth = json!({
        "dsh_gateway_key": Path::new(&home).join(".dsh/openai-gateway/api-key").exists(),
        "codex_auth": Path::new(&home).join(".codex/auth.json").exists(),
        "claude_env": root.join("claude.env").exists(),
    });
    json!({
        "mesh_root": root.display().to_string(),
        "path": path,
        "tools": tool_map,
        "auth": auth,
    })
}

// ---------- run：spawn sidecar + watchdog（§4.8） ----------

struct AppState {
    mesh_root: Mutex<PathBuf>,
    login_path: Mutex<String>,
    db_path: PathBuf,
    projects_root: PathBuf,
}

fn resolve_result(root: &Path, id: &str) -> Value {
    let dir = root.join("results").join(id);
    let result = fs::read_to_string(dir.join("result.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok());
    let answer = fs::read_to_string(dir.join("answer.txt")).unwrap_or_default();
    match result {
        Some(mut r) => {
            r["answer_text"] = Value::from(answer);
            r["synthesized"] = Value::from(false);
            r
        }
        None => json!({
            "status": "failed", "synthesized": true,
            "error": "result.json missing or unparsable after process exit",
            "answer_text": answer,
        }),
    }
}

fn killpg(pgid: u32, sig: i32) {
    unsafe {
        libc::killpg(pgid as i32, sig);
    }
}

#[tauri::command]
fn run_task(task: Value, state: tauri::State<'_, AppState>) -> Result<Value, String> {
    run_task_impl(task, &state)
}

fn run_task_impl(task: Value, state: &AppState) -> Result<Value, String> {
    let id = task
        .get("id")
        .and_then(Value::as_str)
        .ok_or("task.id required")?
        .to_string();
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err("task.id must match [A-Za-z0-9._-]+ (no '..')".into());
    }
    let timeout_s = task
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(300);
    // watchdog budget（§4.8）：app 侧固定 retry.max_attempts=1 → timeout + 30s
    let budget = Duration::from_secs(timeout_s + 30);

    let root = state.mesh_root.lock().unwrap().clone();
    let task_path = root.join("tasks").join(format!("{id}.json"));
    fs::write(&task_path, serde_json::to_string_pretty(&task).map_err(|e| e.to_string())?)
        .map_err(|e| format!("write task: {e}"))?;

    let sidecar = sidecar_path().ok_or("sidecar mesh-run not found in bundle")?;
    let mut cmd = Command::new(&sidecar);
    cmd.arg("worker")
        .arg(&task_path)
        .env("AGENTCHAT_MESH_ROOT", &root)
        .env("PATH", state.login_path.lock().unwrap().clone())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    // 独立进程组：killpg 可连 worker 再 spawn 的子进程一起回收
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
// M0 假 worker 通道：仅 AGENTCHAT_SELFTEST=1|slow|lane 时允许 fake-bin 前置 PATH，
// 避免污染真实聊天/正常使用中的 CLI 解析
let st = std::env::var("AGENTCHAT_SELFTEST").unwrap_or_default();
if st == "1" || st == "slow" || st == "lane" {
let fake = mesh_data().join("fake-bin");
if fake.is_dir() {
let p = format!("{}:{}", fake.display(), state.login_path.lock().unwrap());
cmd.env("PATH", p);
}
}

    let mut child: Child = cmd.spawn().map_err(|e| format!("spawn sidecar: {e}"))?;
    let pgid = child.id();

    // stdout/stderr 异步抽干，防管道缓冲撑满后子进程阻塞
    if let Some(out) = child.stdout.take() {
        thread::spawn(move || {
            let _ = BufReader::new(out).lines().count();
        });
    }
    if let Some(err) = child.stderr.take() {
        thread::spawn(move || {
            let _ = BufReader::new(err).lines().count();
        });
    }

    // watchdog 轮询
    let start = Instant::now();
    let outcome = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status.code().unwrap_or(1)),
            Ok(None) => {
                if start.elapsed() > budget {
                    killpg(pgid, libc::SIGTERM);
                    thread::sleep(Duration::from_secs(5));
                    let _ = child.kill(); // 兜底
                    killpg(pgid, libc::SIGKILL);
                    let _ = child.wait();
                    break Err(());
                }
                thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(format!("wait: {e}")),
        }
    };

    let result_dir = root.join("results").join(&id);
    match outcome {
        Ok(_code) => {
            // K0 原子写保证此处 result.json 要么完整要么不存在
            Ok(resolve_result(&root, &id))
        }
        Err(()) => {
            // 合成 timeout 记录（§4.8）：清理 running 标记，不伪造内核语义
            let _ = fs::remove_file(result_dir.join("running"));
            let answer = fs::read_to_string(result_dir.join("answer.txt")).unwrap_or_default();
            Ok(json!({
                "task_id": id, "status": "timeout", "synthesized": true,
                "duration_s": start.elapsed().as_secs(),
                "answer_text": answer,
                "error": format!("watchdog killpg after budget {}s", budget.as_secs()),
            }))
        }
    }
}

// ---------- M1：绑定 / 房间 / 消息 commands ----------
// DB 连接每次调用独立打开（SQLite WAL 多连接），无跨命令锁竞争

#[tauri::command]
fn bind_personas(engines: Vec<String>, state: tauri::State<'_, AppState>) -> Result<Value, String> {
    Db::open(&state.db_path)?.bind_personas(&engines)
}

#[tauri::command]
fn list_personas(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    Db::open(&state.db_path)?.list_personas()
}

#[tauri::command]
fn list_rooms(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    Db::open(&state.db_path)?.list_rooms()
}

#[tauri::command]
fn open_dm(persona_id: String, state: tauri::State<'_, AppState>) -> Result<Value, String> {
    Db::open(&state.db_path)?.open_dm(&persona_id, &state.projects_root)
}

#[tauri::command]
fn room_history(room_id: String, state: tauri::State<'_, AppState>) -> Result<Value, String> {
    Db::open(&state.db_path)?.room_history(&room_id, 200)
}

#[tauri::command]
fn send_message(room_id: String, text: String, client_msg_id: Option<String>, state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let db = Db::open(&state.db_path)?;
    send_message_core(&db, &state, &room_id, &text, client_msg_id.as_deref())
}

/// 发送消息核心（M2a-2：消息+turn 同事务进房间 lane，DM 单成员闭环；群聊调度随 M3）
fn send_message_core(db: &Db, state: &AppState, room_id: &str, text: &str, client_msg_id: Option<&str>) -> Result<Value, String> {
    // 1) 上下文
    let room = db.get_room(room_id)?;
    let members = room["members"].as_array().cloned().unwrap_or_default();
    let pid = members
        .first()
        .and_then(Value::as_str)
        .ok_or("room has no members")?
        .to_string();
    let persona = db.get_persona(&pid)?;
    let engine = persona["engine"].as_str().unwrap_or("").to_string();
    let cwd = room["cwd"].as_str().unwrap_or("").to_string();

    // 2) 消息+turn 同事务（§6.2.1）。重复 client_msg_id → created=false：同一 turn 绝不双消费（§4.11）
    let (_msg_id, _cmid, turn_id, created) =
        db.add_user_message_turn(room_id, text, client_msg_id, &[pid.clone()])?;
    if !created {
        return db.room_history(room_id, 200);
    }

    // 3) 任务卡（§4.1；session 续接；codex 安全模式字段预置，K3 落地后生效）
    let session = db.last_session(room_id, &pid);
    let run_id = format!("run-{}", db::uuid8());
    let mut task = json!({
        "id": run_id, "worker": engine, "workspace": cwd,
        "timeout_seconds": 300, "brief": text,
    });
    if let Some(s) = session {
        task["session_id"] = Value::from(s);
    }
    if engine == "codex" {
        task["sandbox"] = Value::from("read-only");
    }

    // 4) 执行 + 完成事务（§4.3：run+气泡+turn 终态单事务；执行失败也落 failed 终态，不悬挂 turn）
    let result = run_task_impl(task.clone(), state).unwrap_or_else(|e| json!({
        "task_id": run_id, "status": "failed", "answer_text": "", "error": e,
    }));
    let bubble = bubble_text(&result);
    db.complete_turn(&turn_id, room_id, &pid, &result, &task.to_string(), 1, &bubble)?;
    db.room_history(room_id, 200)
}

/// 气泡文本：无正文输出时落状态占位（与 M1 行为一致）
fn bubble_text(result: &Value) -> String {
    let status = result["status"].as_str().unwrap_or("failed");
    let answer = result["answer_text"].as_str().unwrap_or("");
    if answer.is_empty() {
        format!("[{status}]（无正文输出）")
    } else {
        answer.to_string()
    }
}

/// 启动崩溃恢复（§6.2.1）：只补未完成的 persona——run 成功缺气泡幂等补写；
/// run 缺失/interrupted 按 attempt+1 重跑（新 run_id）。返回执行的恢复动作数。
fn recover_pending_turns(state: &AppState) -> Result<usize, String> {
    let db = Db::open(&state.db_path)?;
    let actions = db.recover_scan()?;
    let mut n = 0;
    for a in actions {
        let action = a["action"].as_str().unwrap_or("");
        let turn_id = a["turn_id"].as_str().unwrap_or("");
        let room_id = a["room_id"].as_str().unwrap_or("");
        let pid = a["persona_id"].as_str().unwrap_or("");
        let attempt = a["attempt"].as_i64().unwrap_or(1);
        match action {
            "backfill" => {
                let run = a["run"].clone();
                db.complete_turn(turn_id, room_id, pid, &run, "{}", attempt, &bubble_text(&run))?;
                n += 1;
            }
            "rerun" => {
                // 快照优先；无快照（崩溃在 run 创建前）按触发原文重建任务卡
                let mut task: Value = a
                    .get("task_json")
                    .and_then(Value::as_str)
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_else(|| {
                        let room = db.get_room(room_id).unwrap_or(Value::Null);
                        let persona = db.get_persona(pid).unwrap_or(Value::Null);
                        json!({
                            "worker": persona["engine"].as_str().unwrap_or(""),
                            "workspace": room["cwd"].as_str().unwrap_or(""),
                            "timeout_seconds": 300,
                            "brief": a["brief"].as_str().unwrap_or(""),
                        })
                    });
                let run_id = format!("run-{}", db::uuid8());
                task["id"] = Value::from(run_id.clone());
                if task.get("session_id").is_none() {
                    if let Some(s) = db.last_session(room_id, pid) {
                        task["session_id"] = Value::from(s);
                    }
                }
                let result = run_task_impl(task.clone(), state).unwrap_or_else(|e| json!({
                    "task_id": run_id, "status": "failed", "answer_text": "", "error": e,
                }));
                let bubble = bubble_text(&result);
                db.complete_turn(turn_id, room_id, pid, &result, &task.to_string(), attempt, &bubble)?;
                n += 1;
            }
            _ => {}
        }
    }
    Ok(n)
}

// ---------- 无头自检（M0 验收：模拟 launchd 环境跑 doctor + 假 worker） ----------
// 用法: env -i HOME=$HOME PATH=/usr/bin:/bin AGENTCHAT_SELFTEST=1 \
//   agentchat.app/Contents/MacOS/agentchat
fn selftest() -> i32 {
    let root = match ensure_kernel(None) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[selftest] ensure_kernel FAILED: {e}");
            return 1;
        }
    };
    let path = login_path();
    println!("[selftest] mesh_root={}", root.display());
    println!("[selftest] login_path={path}");

    let state = AppState {
        mesh_root: Mutex::new(root.clone()),
        login_path: Mutex::new(path),
        db_path: mesh_data().join("agentchat.db"),
        projects_root: projects_root(),
    };

    // M1 聊天闭环验收：AGENTCHAT_SELFTEST=chat 用真 dsh 跑两轮对话
    if std::env::var("AGENTCHAT_SELFTEST").as_deref() == Ok("chat") {
        return selftest_chat(&state);
    }

    // M2a-1 schema 迁移验收：AGENTCHAT_SELFTEST=migrate（可用 AGENTCHAT_MIGRATE_FIXTURE 指定真 M1 库副本）
    if std::env::var("AGENTCHAT_SELFTEST").as_deref() == Ok("migrate") {
        return selftest_migrate();
    }

    // M2a-2 lane 验收：AGENTCHAT_SELFTEST=lane（隔离临时库 + 假 worker）
    if std::env::var("AGENTCHAT_SELFTEST").as_deref() == Ok("lane") {
        return selftest_lane(&state);
    }

    // 假 worker 一轮（fake-bin/dsh 须在 PATH 前置目录中）
    let slow = std::env::var("AGENTCHAT_SELFTEST").as_deref() == Ok("slow");
    let id = format!("m0-selftest-{}", std::process::id());
    let task = if slow {
        json!({
            "id": id, "worker": "dsh",
            "brief": "SLOW fake task", "timeout_seconds": 3,
        })
    } else {
        json!({
            "id": id, "worker": "dsh",
            "brief": "fake ping from selftest", "timeout_seconds": 60,
        })
    };
    let r = run_task_impl(task, &state);
    match r {
        Ok(v) => {
            println!("[selftest] run_task={}", serde_json::to_string_pretty(&v).unwrap());
            let ok = if slow {
                // 两种合法路径（§4.8 超时归属）：
                // A. 内核 timeout 生效 → exit_code=124，result.json 由内核写（synthesized=false）
                // B. 无 timeout 命令 → watchdog killpg → app 合成（synthesized=true）且 running 已清理
                let is_timeout = v.get("status").and_then(Value::as_str) == Some("timeout");
                let kernel_path = v.get("exit_code").and_then(Value::as_i64) == Some(124);
                let watchdog_path = v.get("synthesized").and_then(Value::as_bool) == Some(true)
                    && !root.join("results").join(&id).join("running").exists();
                is_timeout && (kernel_path || watchdog_path)
            } else {
                v.get("status").and_then(Value::as_str) == Some("success")
                    && v.get("answer_text")
                        .and_then(Value::as_str)
                        .map(|a| a.contains("PONG"))
                        .unwrap_or(false)
            };
            // 清理
            let _ = fs::remove_dir_all(root.join("results").join(&id));
            if ok {
                println!("[selftest] PASS");
                0
            } else {
                println!("[selftest] FAIL: unexpected status/answer");
                1
            }
        }
        Err(e) => {
            eprintln!("[selftest] run_task FAILED: {e}");
            1
        }
    }
}

/// M1 验收：绑定 dsh → 开 DM → 两轮真对话 → session 续接 + 历史断言
fn selftest_chat(state: &AppState) -> i32 {
    let db = match Db::open(&state.db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[selftest-chat] db open FAILED: {e}");
            return 1;
        }
    };
    let fail = |msg: &str| {
        eprintln!("[selftest-chat] FAIL: {msg}");
        1
    };

    // 1) 绑定 dsh persona
    let personas = match db.bind_personas(&["dsh".to_string()]) {
        Ok(v) => v,
        Err(e) => return fail(&format!("bind_personas: {e}")),
    };
    let pid = personas[0]["id"].as_str().unwrap_or("").to_string();
    println!("[selftest-chat] persona bound: {pid}");

    // 2) 开 DM（幂等：复用已有房间）
    let room = match db.open_dm(&pid, &state.projects_root) {
        Ok(v) => v,
        Err(e) => return fail(&format!("open_dm: {e}")),
    };
    let room_id = room["id"].as_str().unwrap_or("").to_string();
    println!("[selftest-chat] dm room: {room_id} cwd={}", room["cwd"]);

    // 3) 第一轮：建立 session
    let h1 = match send_message_core(&db, state, &room_id, "Reply with exactly one word: PONG. No punctuation.", None) {
        Ok(v) => v,
        Err(e) => return fail(&format!("round1: {e}")),
    };
    let s1 = db.last_session(&room_id, &pid);
    println!("[selftest-chat] round1 done, session_id={s1:?}");
    if s1.is_none() {
        return fail("round1 未产生 session_id（dsh session 提取失败）");
    }
    let r1_ok = h1
        .as_array()
        .map(|a| a.iter().any(|m| m["role"] == "persona" && m["status"] == "success"))
        .unwrap_or(false);
    if !r1_ok {
        return fail("round1 无成功 persona 气泡");
    }

    // 4) 第二轮：验证续接（last_session 应返回第一轮的 session_id 并注入任务卡）
    let before = db
        .last_session(&room_id, &pid)
        .expect("session after round1");
    let h2 = match send_message_core(&db, state, &room_id, "What did I ask you to reply in my previous message? Answer with that exact word only.", None) {
        Ok(v) => v,
        Err(e) => return fail(&format!("round2: {e}")),
    };
    // 断言：第二轮 run 的任务卡带了 session_id
    let resumed = h2
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|m| m["role"] == "persona")
                .last()
                .map(|m| m["session_id"].as_str().is_some())
                .unwrap_or(false)
        })
        .unwrap_or(false);
    let after = db.last_session(&room_id, &pid);
    println!("[selftest-chat] round2 done, session {before} -> {after:?}, resumed={resumed}");
    if !resumed {
        return fail("round2 未携带 session_id 续聊");
    }

    // 5) 历史完整性：≥4 条消息（2 user + 2 persona）
    let n = h2.as_array().map(|a| a.len()).unwrap_or(0);
    println!("[selftest-chat] history messages={n}");
    if n < 4 {
        return fail(&format!("history 仅 {n} 条，期望 ≥4"));
    }
    println!("[selftest-chat] PASS");
    0
}

fn projects_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    let p = PathBuf::from(home).join("AgentChatProjects");
    let _ = fs::create_dir_all(&p);
    p
}

// ---------- 入口 ----------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if std::env::var("AGENTCHAT_SELFTEST").is_ok() {
        std::process::exit(selftest());
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let root = ensure_kernel(Some(app.handle()))?;
            let path = login_path();
            let db_path = mesh_data().join("agentchat.db");
            Db::open(&db_path)?; // 提前触发 migrate
            let state = AppState {
                mesh_root: Mutex::new(root),
                login_path: Mutex::new(path),
                db_path,
                projects_root: projects_root(),
            };
            // M2a-2 启动崩溃恢复：只补未完成 persona（补气泡/重跑缺失 run）
            match recover_pending_turns(&state) {
                Ok(n) if n > 0 => eprintln!("[agentchat] startup recovery: {n} action(s) replayed"),
                Ok(_) => {}
                Err(e) => eprintln!("[agentchat] startup recovery failed: {e}"),
            }
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            doctor, run_task, bind_personas, list_personas, list_rooms, open_dm, room_history,
            send_message
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// M2a-1 验收：v0 旧库 → v1 迁移全路径
/// 1) 正向：fixture（或 AGENTCHAT_MIGRATE_FIXTURE 指定的真 M1 库副本）迁移成功，数据/会话/气泡不丢
/// 2) 逆向：注入中途失败 → 报错且提示备份 → 从备份恢复 → 重开迁移成功
fn selftest_migrate() -> i32 {
    let fail = |msg: &str| {
        eprintln!("[selftest-migrate] FAIL: {msg}");
        1
    };
    let dir = std::env::temp_dir().join(format!("agentchat-migrate-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    if let Err(e) = fs::create_dir_all(&dir) {
        return fail(&format!("mkdir: {e}"));
    }
    let db_path = dir.join("agentchat.db");

    // —— 造 v0 fixture（或复制真实 M1 库）——
    if let Ok(src) = std::env::var("AGENTCHAT_MIGRATE_FIXTURE") {
        if let Err(e) = fs::copy(&src, &db_path) {
            return fail(&format!("copy fixture {src}: {e}"));
        }
        println!("[selftest-migrate] fixture = real M1 db copy ({src})");
    } else if let Err(e) = build_v0_fixture(&db_path) {
        return fail(&format!("build fixture: {e}"));
    }

    // —— 正向迁移 ——
    let db = match Db::open(&db_path) {
        Ok(d) => d,
        Err(e) => return fail(&format!("migrate: {e}")),
    };
    if let Err(e) = verify_migrated(&db, &db_path) {
        return fail(&e);
    }
    println!("[selftest-migrate] forward migration OK");
    drop(db);

    // —— 逆向：注入失败 → 回滚 → 备份恢复 → 重迁 ——
    let _ = fs::remove_file(&db_path);
    let _ = fs::remove_file(dir.join("agentchat.db-wal"));
    let _ = fs::remove_file(dir.join("agentchat.db-shm"));
    if let Ok(src) = std::env::var("AGENTCHAT_MIGRATE_FIXTURE") {
        let _ = fs::copy(&src, &db_path);
    } else if let Err(e) = build_v0_fixture(&db_path) {
        return fail(&format!("rebuild fixture: {e}"));
    }
    // 自测进程单线程，env 注入安全
    unsafe { std::env::set_var("AGENTCHAT_MIGRATE_FAIL_AFTER", "rooms") };
    let injected = match Db::open(&db_path) {
        Ok(_) => return fail("注入失败却迁移成功"),
        Err(e) => e,
    };
    unsafe { std::env::remove_var("AGENTCHAT_MIGRATE_FAIL_AFTER") };
    if !injected.contains("备份在") {
        return fail(&format!("失败信息未带备份路径: {injected}"));
    }
    // 模拟用户从备份恢复（备份文件名见 backups/ 目录最新一份）
    let backups_dir = dir.join("backups");
    let mut backups: Vec<_> = fs::read_dir(&backups_dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|x| x.path())).collect())
        .unwrap_or_default();
    backups.sort();
    let Some(backup) = backups.last() else {
        return fail("失败后未找到备份文件");
    };
    let _ = fs::remove_file(dir.join("agentchat.db-wal"));
    let _ = fs::remove_file(dir.join("agentchat.db-shm"));
    if let Err(e) = fs::copy(backup, &db_path) {
        return fail(&format!("restore from backup: {e}"));
    }
    let db = match Db::open(&db_path) {
        Ok(d) => d,
        Err(e) => return fail(&format!("re-migrate after restore: {e}")),
    };
    if let Err(e) = verify_migrated(&db, &db_path) {
        return fail(&format!("after restore: {e}"));
    }
    println!("[selftest-migrate] failure→rollback→restore→re-migrate OK");
    println!("[selftest-migrate] PASS");
    0
}

/// 造一个 v0（M1 旧 schema）fixture 库：1 persona + 1 DM 房 + 4 消息 + 2 成功 run（含 session）
fn build_v0_fixture(db_path: &Path) -> Result<(), String> {
    let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
CREATE TABLE personas(
  id TEXT PRIMARY KEY, engine TEXT NOT NULL, name TEXT NOT NULL,
  color TEXT NOT NULL, bound INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL);
CREATE TABLE rooms(
  id TEXT PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL,
  members TEXT NOT NULL, cwd TEXT NOT NULL, permission TEXT NOT NULL DEFAULT 'project',
  default_target TEXT NOT NULL DEFAULT 'last', max_auto_turns INTEGER NOT NULL DEFAULT 6,
  dm_settings TEXT, created_at TEXT NOT NULL);
CREATE TABLE messages(
  id INTEGER PRIMARY KEY AUTOINCREMENT, room_id TEXT NOT NULL, sender TEXT NOT NULL,
  role TEXT NOT NULL, text TEXT NOT NULL, run_id TEXT, reply_to INTEGER, created_at TEXT NOT NULL);
CREATE INDEX idx_messages_room ON messages(room_id, id);
CREATE TABLE runs(
  id TEXT PRIMARY KEY, room_id TEXT, persona_id TEXT, task_json TEXT NOT NULL,
  status TEXT, session_id TEXT, tokens INTEGER, duration_s REAL, synthesized INTEGER DEFAULT 0,
  answer_text TEXT, created_at TEXT NOT NULL, finished_at TEXT);
CREATE INDEX idx_runs_room ON runs(room_id, persona_id, finished_at);
CREATE TABLE run_nodes(
  id TEXT PRIMARY KEY, run_id TEXT NOT NULL, node_seq INTEGER, persona_id TEXT,
  depends_on TEXT, status TEXT, winner INTEGER DEFAULT 0);
CREATE TABLE scores(
  id INTEGER PRIMARY KEY AUTOINCREMENT, persona_id TEXT NOT NULL, room_id TEXT,
  event TEXT NOT NULL, delta INTEGER NOT NULL, detail TEXT, created_at TEXT NOT NULL);
INSERT INTO personas VALUES('p-fixture1','dsh','DeepSeek Harness','#0a84ff',1,'2026-11-26T10:00:00Z');
INSERT INTO rooms(id,kind,name,members,cwd,permission,default_target,max_auto_turns,created_at)
  VALUES('r-fixture1','dm','DeepSeek Harness','[\"p-fixture1\"]','/tmp/fixture-cwd','project','last',6,'2026-11-26T10:00:01Z');
INSERT INTO messages(room_id,sender,role,text,run_id,created_at) VALUES
  ('r-fixture1','user','user','hi',NULL,'2026-11-26T10:00:10Z'),
  ('r-fixture1','p-fixture1','persona','PONG','msg-run1','2026-11-26T10:00:20Z'),
  ('r-fixture1','user','user','what did I say',NULL,'2026-11-26T10:00:30Z'),
  ('r-fixture1','p-fixture1','persona','PONG','msg-run2','2026-11-26T10:00:40Z');
INSERT INTO runs(id,room_id,persona_id,task_json,status,session_id,tokens,duration_s,synthesized,answer_text,created_at,finished_at) VALUES
  ('msg-run1','r-fixture1','p-fixture1','{}','success','session-fixture-1',120,2.5,0,'PONG','2026-11-26T10:00:11Z','2026-11-26T10:00:20Z'),
  ('msg-run2','r-fixture1','p-fixture1','{}','success','session-fixture-1',98,2.1,0,'PONG','2026-11-26T10:00:31Z','2026-11-26T10:00:40Z');",
    )
    .map_err(|e| e.to_string())?;
    // 确保 WAL 模式落盘（验收含 WAL 库迁移）
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if mode != "wal" {
        return Err(format!("fixture not in WAL mode: {mode}"));
    }
    Ok(())
}

/// 迁移后全量断言（§4.10 验收：历史可打开、气泡 status/tokens 不丢、session 不丢）
fn verify_migrated(db: &Db, db_path: &Path) -> Result<(), String> {
    // user_version = 1
    let v: i64 = db
        .0
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if v != 1 {
        return Err(format!("user_version={v} != 1"));
    }
    // messages：id 保留 + client_msg_id backfill + legacy_run_id 保留
    let msgs: Vec<(i64, String, Option<String>)> = db
        .0
        .prepare("SELECT id, client_msg_id, legacy_run_id FROM messages ORDER BY id")
        .map_err(|e| e.to_string())?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|x| x.ok())
        .collect();
    if msgs.is_empty() {
        return Err("messages empty after migration".into());
    }
    for (id, cmid, legacy) in &msgs {
        if *cmid != format!("msg-legacy-{id}") {
            return Err(format!("client_msg_id backfill wrong: {cmid}"));
        }
        let _ = legacy; // 有 run 的气泡应保留 legacy_run_id（下方 history 断言覆盖）
    }
    // room_history：persona 气泡 status/tokens 不丢（legacy_run_id join 路径）
    let room_id: String = db
        .0
        .query_row("SELECT id FROM rooms LIMIT 1", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let history = db.room_history(&room_id, 200)?;
    let bubbles: Vec<_> = history
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m["role"] == "persona")
        .collect();
    if bubbles.is_empty() {
        return Err("no persona bubbles in history".into());
    }
    for b in &bubbles {
        if b["status"].as_str() != Some("success") {
            return Err(format!("bubble lost status: {b}"));
        }
        if b["tokens"].as_i64().is_none() {
            return Err(format!("bubble lost tokens: {b}"));
        }
    }
    // room_persona_sessions backfill + last_session（K3 权威表）
    let pid = bubbles[0]["sender"].as_str().unwrap_or("").to_string();
    if db.last_session(&room_id, &pid).is_none() {
        return Err("room_persona_sessions not backfilled / last_session lost".into());
    }
    // room_members backfill + get_room members
    let room = db.get_room(&room_id)?;
    if room["members"].as_array().map(|a| a.is_empty()).unwrap_or(true) {
        return Err("room_members not backfilled".into());
    }
    if room["owner_persona_id"].as_str().is_none() {
        return Err("owner_persona_id not backfilled".into());
    }
    // 新表齐备
    for t in ["turns", "room_summaries", "approvals", "room_persona_sessions", "room_members"] {
        let n: i64 = db
            .0
            .query_row(
                &format!("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='{t}'"),
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if n != 1 {
            return Err(format!("table {t} missing"));
        }
    }
    // runs 部分唯一索引存在
    let n: i64 = db
        .0
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='uq_runs_turn'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if n != 1 {
        return Err("uq_runs_turn index missing".into());
    }
    // 备份存在且目录 0700
    let backups = db_path.parent().unwrap().join("backups");
    let meta = fs::metadata(&backups).map_err(|e| format!("backups dir: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o777 != 0o700 {
            return Err("backups dir not 0700".into());
        }
    }
    let has_backup = fs::read_dir(&backups)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().map(|x| x == "db").unwrap_or(false));
    if !has_backup {
        return Err("no backup file created".into());
    }
    // client_msg_id 自铸格式（msg-<26 字符 ulid>）
    let (_, cmid) = db.add_message(&room_id, "user", "user", "selftest ulid", None, None, None)?;
    if !cmid.starts_with("msg-") || cmid.len() != 30 {
        return Err(format!("client_msg_id format wrong: {cmid}"));
    }
    Ok(())
}

/// M2a-2 验收：lane + 三层幂等 + 崩溃恢复（隔离临时库，假 worker）
/// 用例1：重复 client_msg_id → 不产生重复气泡/turn（第一层幂等）
/// 用例2：崩溃于 run 成功后气泡前 → 恢复幂等补写气泡，不重跑（§4.3 完成事务恢复）
/// 用例3：崩溃于 run 创建前 → 恢复按触发原文重建任务卡重跑，只补缺失 persona
/// 用例4：run 失败/interrupted → 恢复按 attempt+1 重跑（§4.3 重试契约）
fn selftest_lane(state0: &AppState) -> i32 {
    let fail = |msg: &str| {
        eprintln!("[selftest-lane] FAIL: {msg}");
        1
    };
    let dir = std::env::temp_dir().join(format!("agentchat-lane-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    if let Err(e) = fs::create_dir_all(&dir) {
        return fail(&format!("mkdir: {e}"));
    }
    let db_path = dir.join("agentchat.db");
    let state = AppState {
        mesh_root: Mutex::new(state0.mesh_root.lock().unwrap().clone()),
        login_path: Mutex::new(state0.login_path.lock().unwrap().clone()),
        db_path: db_path.clone(),
        projects_root: dir.join("projects"),
    };
    let db = match Db::open(&db_path) {
        Ok(d) => d,
        Err(e) => return fail(&format!("db open: {e}")),
    };
    let personas = match db.bind_personas(&["dsh".to_string()]) {
        Ok(v) => v,
        Err(e) => return fail(&format!("bind_personas: {e}")),
    };
    let pid = personas[0]["id"].as_str().unwrap_or("").to_string();
    let room = match db.open_dm(&pid, &state.projects_root) {
        Ok(v) => v,
        Err(e) => return fail(&format!("open_dm: {e}")),
    };
    let room_id = room["id"].as_str().unwrap_or("").to_string();
    println!("[selftest-lane] room={room_id} persona={pid}");

    let count = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> i64 {
        db.0.query_row(sql, rusqlite::params_from_iter(p.iter()), |r| r.get(0))
            .unwrap_or(-1)
    };

    // —— 用例1：重复 client_msg_id 幂等 ——
    let cmid = "msg-LANEDUP000000000000000001";
    let h1 = match send_message_core(&db, &state, &room_id, "lane dup test", Some(cmid)) {
        Ok(v) => v,
        Err(e) => return fail(&format!("case1 first send: {e}")),
    };
    let h2 = match send_message_core(&db, &state, &room_id, "lane dup test", Some(cmid)) {
        Ok(v) => v,
        Err(e) => return fail(&format!("case1 dup send: {e}")),
    };
    let user_msgs = count(
        "SELECT COUNT(*) FROM messages WHERE room_id=? AND client_msg_id=?",
        &[&room_id, &cmid],
    );
    let persona_bubbles = count(
        "SELECT COUNT(*) FROM messages WHERE room_id=? AND role='persona'",
        &[&room_id],
    );
    let turns_n = count("SELECT COUNT(*) FROM turns WHERE room_id=?", &[&room_id]);
    println!("[selftest-lane] case1: history {} vs {}, user_msgs={user_msgs}, bubbles={persona_bubbles}, turns={turns_n}",
        h1.as_array().map(|a| a.len()).unwrap_or(0),
        h2.as_array().map(|a| a.len()).unwrap_or(0));
    if user_msgs != 1 || persona_bubbles != 1 || turns_n != 1 {
        return fail("case1: 重复 client_msg_id 产生了重复消息/气泡/turn");
    }

    // —— 用例2：run 成功但气泡缺失 → 幂等补写，不重跑 ——
    let (_m2, _c2, turn2, created2) =
        match db.add_user_message_turn(&room_id, "crash after run", None, &[pid.clone()]) {
            Ok(v) => v,
            Err(e) => return fail(&format!("case2 turn: {e}")),
        };
    if !created2 {
        return fail("case2: turn 未创建");
    }
    let run2 = json!({"task_id": "run-crash2", "status": "success", "answer_text": "CRASH2-ANSWER"});
    if let Err(e) = db.add_run(&run2, &room_id, &pid, "{}", Some(&turn2), 1) {
        return fail(&format!("case2 add_run: {e}"));
    }
    // 模拟崩溃：run 已落库，气泡与 turn 终态未写
    let n = match recover_pending_turns(&state) {
        Ok(v) => v,
        Err(e) => return fail(&format!("case2 recover: {e}")),
    };
    let bubbles2 = count(
        "SELECT COUNT(*) FROM messages WHERE room_id=? AND turn_id=? AND sender=?",
        &[&room_id, &turn2, &pid],
    );
    let turn2_status: String = db
        .0
        .query_row(
            "SELECT status FROM turns WHERE turn_id=?",
            rusqlite::params![turn2],
            |r| r.get(0),
        )
        .unwrap_or_default();
    println!("[selftest-lane] case2: recovery actions={n}, bubbles={bubbles2}, turn={turn2_status}");
    if n != 1 || bubbles2 != 1 || turn2_status != "done" {
        return fail("case2: 气泡补写/turn 终态不符预期");
    }
    // 幂等：再次恢复 0 动作，气泡仍只有一条，run 仍只有一次（未重跑）
    let n2 = match recover_pending_turns(&state) {
        Ok(v) => v,
        Err(e) => return fail(&format!("case2 recover#2: {e}")),
    };
    let runs2 = count(
        "SELECT COUNT(*) FROM runs WHERE turn_id=? AND persona_id=?",
        &[&turn2, &pid],
    );
    let bubbles2b = count(
        "SELECT COUNT(*) FROM messages WHERE room_id=? AND turn_id=? AND sender=?",
        &[&room_id, &turn2, &pid],
    );
    if n2 != 0 || runs2 != 1 || bubbles2b != 1 {
        return fail("case2: 恢复不幂等（重复补写/重跑）");
    }

    // —— 用例3：崩溃于 run 创建前 → 重建任务卡重跑 ——
    let (_m3, _c3, turn3, _) =
        match db.add_user_message_turn(&room_id, "crash before run", None, &[pid.clone()]) {
            Ok(v) => v,
            Err(e) => return fail(&format!("case3 turn: {e}")),
        };
    let n3 = match recover_pending_turns(&state) {
        Ok(v) => v,
        Err(e) => return fail(&format!("case3 recover: {e}")),
    };
    let runs3 = count(
        "SELECT COUNT(*) FROM runs WHERE turn_id=? AND persona_id=? AND status='success'",
        &[&turn3, &pid],
    );
    let bubbles3 = count(
        "SELECT COUNT(*) FROM messages WHERE room_id=? AND turn_id=? AND sender=?",
        &[&room_id, &turn3, &pid],
    );
    let turn3_status: String = db
        .0
        .query_row(
            "SELECT status FROM turns WHERE turn_id=?",
            rusqlite::params![turn3],
            |r| r.get(0),
        )
        .unwrap_or_default();
    println!("[selftest-lane] case3: recovery actions={n3}, success runs={runs3}, bubbles={bubbles3}, turn={turn3_status}");
    if n3 != 1 || runs3 != 1 || bubbles3 != 1 || turn3_status != "done" {
        return fail("case3: 缺失 run 重跑/终态不符预期");
    }

    // —— 用例4：failed run → attempt+1 重跑（第三层幂等：(turn_id,persona_id,attempt)）——
    let (_m4, _c4, turn4, _) =
        match db.add_user_message_turn(&room_id, "retry after fail", None, &[pid.clone()]) {
            Ok(v) => v,
            Err(e) => return fail(&format!("case4 turn: {e}")),
        };
    let run4 = json!({"task_id": "run-fail4", "status": "failed", "answer_text": ""});
    let snap4 = json!({"id": "run-fail4", "worker": "dsh", "timeout_seconds": 60, "brief": "retry after fail"});
    if let Err(e) = db.add_run(&run4, &room_id, &pid, &snap4.to_string(), Some(&turn4), 1) {
        return fail(&format!("case4 add_run: {e}"));
    }
    let n4 = match recover_pending_turns(&state) {
        Ok(v) => v,
        Err(e) => return fail(&format!("case4 recover: {e}")),
    };
    let attempt2_ok = count(
        "SELECT COUNT(*) FROM runs WHERE turn_id=? AND persona_id=? AND attempt=2 AND status='success'",
        &[&turn4, &pid],
    );
    let turn4_status: String = db
        .0
        .query_row(
            "SELECT status FROM turns WHERE turn_id=?",
            rusqlite::params![turn4],
            |r| r.get(0),
        )
        .unwrap_or_default();
    println!("[selftest-lane] case4: recovery actions={n4}, attempt2 success={attempt2_ok}, turn={turn4_status}");
    if n4 != 1 || attempt2_ok != 1 || turn4_status != "done" {
        return fail("case4: attempt+1 重跑不符预期");
    }
    // 最终幂等：全库再无悬挂 turn
    let dangling = count(
        "SELECT COUNT(*) FROM turns WHERE status IN ('claimed','running')",
        &[],
    );
    if dangling != 0 {
        return fail("final: 仍有悬挂 turn");
    }

    let _ = fs::remove_dir_all(&dir);
    println!("[selftest-lane] PASS (4 cases)");
    0
}
