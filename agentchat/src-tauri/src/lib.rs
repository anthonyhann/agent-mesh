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
// M0 假 worker 通道：仅 AGENTCHAT_SELFTEST=1|slow 时允许 fake-bin 前置 PATH，
// 避免污染真实聊天/正常使用中的 CLI 解析
let st = std::env::var("AGENTCHAT_SELFTEST").unwrap_or_default();
if st == "1" || st == "slow" {
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
fn send_message(room_id: String, text: String, state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let db = Db::open(&state.db_path)?;
    send_message_core(&db, &state, &room_id, &text)
}

/// 发送消息核心（M1：DM 单成员闭环；群聊 default_target 调度随 M3）
fn send_message_core(db: &Db, state: &AppState, room_id: &str, text: &str) -> Result<Value, String> {
    // 1) 上下文 + 用户消息落库
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
    let session = db.last_session(room_id, &pid);
    db.add_message(room_id, "user", "user", text, None)?;

    // 2) 任务卡（§4.1；session 续接；codex 安全模式字段预置，K3 落地后生效）
    let run_id = format!("msg-{}", db::uuid8());
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

    // 3) 执行 + 落库（本地 Connection 跨 run 无妨，其他命令用自己的连接）
    let result = run_task_impl(task.clone(), state)?;
    db.add_run(&result, room_id, &pid, &task.to_string())?;
    let status = result["status"].as_str().unwrap_or("failed");
    let answer = result["answer_text"].as_str().unwrap_or("").to_string();
    let bubble = if answer.is_empty() {
        format!("[{status}]（无正文输出）")
    } else {
        answer
    };
    db.add_message(room_id, &pid, "persona", &bubble, Some(&run_id))?;
    db.room_history(room_id, 200)
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
    let h1 = match send_message_core(&db, state, &room_id, "Reply with exactly one word: PONG. No punctuation.") {
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
    let h2 = match send_message_core(&db, state, &room_id, "What did I ask you to reply in my previous message? Answer with that exact word only.") {
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
            let db = Db::open(&mesh_data().join("agentchat.db"))?;
            app.manage(AppState {
                mesh_root: Mutex::new(root),
                login_path: Mutex::new(path),
                db_path: mesh_data().join("agentchat.db"),
                projects_root: projects_root(),
            });
            let _ = db; // 提前触发 migrate
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            doctor, run_task, bind_personas, list_personas, list_rooms, open_dm, room_history,
            send_message
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
