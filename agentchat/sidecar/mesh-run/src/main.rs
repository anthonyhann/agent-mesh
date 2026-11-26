//! mesh-run — agent-mesh 内核 sidecar launcher（AgentChat M0）
//!
//! 子命令：
//!   mesh-run worker   <task.json>      → exec zsh  <MESH_ROOT>/run_worker.sh
//!   mesh-run pipeline <manifest.json>  → exec zsh  <MESH_ROOT>/run_pipeline.sh
//!   mesh-run dag      <manifest.json>  → exec python3 <MESH_ROOT>/run_dag.py
//!   mesh-run root                      → 打印解析出的 MESH_ROOT（doctor 用）
//!
//! MESH_ROOT 解析优先级：
//!   1. env AGENTCHAT_MESH_ROOT
//!   2. ~/Library/Application Support/AgentChat/kernel/current（symlink 或目录）
//!
//! 环境注入：宿主进程（Tauri Rust 壳）负责解析 PATH 与 MESH_ENV_EXTRA_VARS，
//! launcher 只透传并做最小校验，本身不读 shell 配置（launchd 环境无 ~/.zshrc）。

use std::env;
use std::path::PathBuf;
use std::process::{exit, Command, Stdio};

fn mesh_root() -> PathBuf {
    if let Ok(p) = env::var("AGENTCHAT_MESH_ROOT") {
        return PathBuf::from(p);
    }
    let home = env::var("HOME").unwrap_or_else(|_| {
        eprintln!("[mesh-run] FATAL: HOME not set");
        exit(2);
    });
    PathBuf::from(home)
        .join("Library/Application Support/AgentChat/kernel/current")
}

fn run(mut cmd: Command) -> ! {
    // stdin 封死：任务全部由任务卡提供（与 run_worker 约定一致）
    cmd.stdin(Stdio::null());
    let status = cmd.status().unwrap_or_else(|e| {
        eprintln!("[mesh-run] FATAL: spawn failed: {e}");
        exit(2);
    });
    exit(status.code().unwrap_or(1));
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let sub = args.first().map(String::as_str).unwrap_or("");
    let root = mesh_root();

    match sub {
        "root" => {
            println!("{}", root.display());
        }
        "worker" | "pipeline" | "dag" => {
            let manifest = args.get(1).unwrap_or_else(|| {
                eprintln!("[mesh-run] FATAL: {sub} requires a manifest path");
                exit(2);
            });
            let (prog, script) = match sub {
                "dag" => ("python3", "run_dag.py"),
                "worker" => ("zsh", "run_worker.sh"),
                _ => ("zsh", "run_pipeline.sh"),
            };
            let script_path = root.join(script);
            if !script_path.exists() {
                eprintln!(
                    "[mesh-run] FATAL: {} not found under MESH_ROOT={}",
                    script,
                    root.display()
                );
                exit(2);
            }
            let mut cmd = Command::new(prog);
            cmd.arg(&script_path).arg(manifest);
            // 内核脚本自定位 MESH_ROOT=$(dirname $0)，工作目录给 MESH_ROOT 保持一致性
            cmd.current_dir(&root);
            run(cmd);
        }
        other => {
            eprintln!("[mesh-run] usage: mesh-run <worker|pipeline|dag|root> [manifest]");
            if !other.is_empty() {
                eprintln!("[mesh-run] unknown subcommand: {other}");
            }
            exit(2);
        }
    }
}
