use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::bd::Issue;
use crate::core::Core;

// --- Types ---

/// output.jsonl のパース結果
#[derive(Clone)]
pub struct ResultData {
    pub result: String,
    pub session_id: Option<String>,
}

/// ジョブのメタ情報（meta.json に永続化）
#[derive(Debug, Serialize, Deserialize)]
pub struct JobMeta {
    pub issue_id: String,
    pub workflow: String,
    pub worktree_path: Option<String>,
    pub started_at: String,
}

/// setup() の戻り値
pub struct SetupContext {
    pub worktree_path: Option<String>,
}

// --- Trait ---

/// workflow 固有のロジックを定義する trait
pub trait WorkflowHandler: Send + Sync + 'static {
    type Event: Send + 'static;
    /// workflow 開始時に渡す追加設定（impl: epic_id 等、enrich/split: ()）
    type Config: Send + 'static;

    fn workflow_name(&self) -> &str;

    /// claude に渡すコマンドライン引数を構築
    fn build_command(&self, issue: &Issue, config: &Self::Config) -> Vec<String>;

    /// claude を実行するディレクトリ
    fn working_dir(&self, meta: &JobMeta) -> PathBuf;

    /// 開始前のセットアップ（impl: worktree 作成、enrich/split: なし）
    fn setup(
        &self,
        issue: &Issue,
        config: &Self::Config,
    ) -> impl std::future::Future<Output = Result<SetupContext>> + Send;

    /// 開始イベントを生成
    fn on_started(&self, issue_id: &str) -> Self::Event;

    /// 完了時: 結果を解釈して Event を生成 + 後処理（bd update 等）
    fn on_completed(
        &self,
        result: ResultData,
        meta: &JobMeta,
    ) -> impl std::future::Future<Output = Self::Event> + Send;

    /// 失敗時
    fn on_failed(&self, issue_id: &str, error: String) -> Self::Event;

    /// session_id 早期発見時（デフォルト: 何もしない）
    fn on_session_id_discovered(
        &self,
        _issue_id: &str,
        _session_id: String,
    ) -> Option<Self::Event> {
        None
    }
}

// --- .strand/ ディレクトリ管理 ---

/// .strand/jobs/ ディレクトリを確保し、.gitignore を自動生成
pub fn ensure_strand_dir() -> Result<PathBuf> {
    let repo_dir = Core::repo_dir();
    let strand_dir = repo_dir.join(".strand");
    let jobs_dir = strand_dir.join("jobs");

    if !jobs_dir.exists() {
        fs::create_dir_all(&jobs_dir)?;
    }

    let gitignore = strand_dir.join(".gitignore");
    if !gitignore.exists() {
        fs::write(&gitignore, "*\n!.gitignore\n")?;
    }

    Ok(jobs_dir)
}

/// workflow と short_id から job ディレクトリパスを生成
pub fn job_dir_path(jobs_dir: &Path, workflow: &str, short_id: &str) -> PathBuf {
    jobs_dir.join(format!("{workflow}-{short_id}"))
}

// --- デタッチ起動 ---

/// プロセスをデタッチ起動し、PID を返す
pub fn spawn_detached(
    args: &[String],
    cwd: &Path,
    stdout_path: &Path,
    stderr_path: &Path,
) -> Result<u32> {
    let stdout_file = fs::File::create(stdout_path).context("failed to create stdout file")?;
    let stderr_file = fs::File::create(stderr_path).context("failed to create stderr file")?;

    let mut child = unsafe {
        Command::new(&args[0])
            .args(&args[1..])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(stdout_file)
            .stderr(stderr_file)
            .pre_exec(|| {
                libc::setsid();
                Ok(())
            })
            .spawn()
            .context("failed to spawn detached process")?
    };

    let pid = child.id();

    // ゾンビ防止: 別スレッドで wait() してプロセスを回収する
    std::thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(pid)
}

// --- PID 管理 ---

pub fn write_pid(job_dir: &Path, pid: u32) -> Result<()> {
    fs::write(job_dir.join("pid"), pid.to_string())?;
    Ok(())
}

pub fn read_pid(job_dir: &Path) -> Result<u32> {
    let s = fs::read_to_string(job_dir.join("pid"))?;
    Ok(s.trim().parse()?)
}

pub fn is_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

// --- Meta 管理 ---

pub fn write_meta(job_dir: &Path, meta: &JobMeta) -> Result<()> {
    let json = serde_json::to_string_pretty(meta)?;
    fs::write(job_dir.join("meta.json"), json)?;
    Ok(())
}

pub fn read_meta(job_dir: &Path) -> Result<JobMeta> {
    let s = fs::read_to_string(job_dir.join("meta.json"))?;
    Ok(serde_json::from_str(&s)?)
}

// --- Output パース ---

/// output.jsonl から最後の "type":"result" 行をパースして ResultData を返す
pub fn parse_output(output_path: &Path) -> Option<ResultData> {
    let file = fs::File::open(output_path).ok()?;
    let reader = BufReader::new(file);

    let mut last_result: Option<ResultData> = None;

    for line in reader.lines() {
        let line = line.ok()?;
        let v: serde_json::Value = serde_json::from_str(&line).ok()?;

        if v.get("type").and_then(|t| t.as_str()) == Some("result") {
            last_result = Some(ResultData {
                result: v
                    .get("result")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string(),
                session_id: v
                    .get("session_id")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
            });
        }
    }

    last_result
}

/// output.jsonl の先頭行から session_id を抽出する
pub fn parse_early_session_id(output_path: &Path) -> Option<String> {
    let file = fs::File::open(output_path).ok()?;
    let reader = BufReader::new(file);
    let first_line = reader.lines().next()?.ok()?;
    let v: serde_json::Value = serde_json::from_str(&first_line).ok()?;
    v.get("session_id")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
}

// --- 高レベル操作 ---

/// ジョブを開始する（全 workflow 共通）
pub async fn start_job<W: WorkflowHandler>(
    handler: &Arc<W>,
    issue: &Issue,
    config: &W::Config,
    tx: &mpsc::Sender<W::Event>,
) -> Result<JobMeta> {
    let jobs_dir = ensure_strand_dir()?;
    let short_id = crate::bd::short_id(&issue.id);
    let job_dir = job_dir_path(&jobs_dir, handler.workflow_name(), short_id);

    // 排他チェック
    if job_dir.exists() {
        anyhow::bail!("already running: {}", issue.id);
    }

    // セットアップ（impl: worktree 作成）
    let setup_ctx = handler.setup(issue, config).await?;

    // job ディレクトリ作成
    fs::create_dir_all(&job_dir)?;

    let now = chrono::Utc::now().to_rfc3339();
    let meta = JobMeta {
        issue_id: issue.id.clone(),
        workflow: handler.workflow_name().to_string(),
        worktree_path: setup_ctx.worktree_path,
        started_at: now,
    };
    write_meta(&job_dir, &meta)?;

    // コマンド構築 + デタッチ起動
    let args = handler.build_command(issue, config);
    let cwd = handler.working_dir(&meta);
    let stdout_path = job_dir.join("output.jsonl");
    let stderr_path = job_dir.join("stderr.log");

    let pid = spawn_detached(&args, &cwd, &stdout_path, &stderr_path)?;
    write_pid(&job_dir, pid)?;

    // Started イベント送信
    let _ = tx.send(handler.on_started(&issue.id)).await;

    // Monitor 起動
    spawn_monitor(
        Arc::clone(handler),
        job_dir.clone(),
        issue.id.clone(),
        tx.clone(),
    );

    Ok(meta)
}

/// monitor タスクを起動する
fn spawn_monitor<W: WorkflowHandler>(
    handler: Arc<W>,
    job_dir: PathBuf,
    issue_id: String,
    tx: mpsc::Sender<W::Event>,
) {
    tokio::spawn(async move {
        monitor_job(handler, &job_dir, &issue_id, &tx).await;
    });
}

/// ジョブを監視する（全 workflow 共通）
async fn monitor_job<W: WorkflowHandler>(
    handler: Arc<W>,
    job_dir: &Path,
    issue_id: &str,
    tx: &mpsc::Sender<W::Event>,
) {
    let mut session_id_sent = false;

    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;

        let pid = match read_pid(job_dir) {
            Ok(p) => p,
            Err(_) => break,
        };

        // session_id の早期取得（1回だけ）
        if !session_id_sent {
            let output_path = job_dir.join("output.jsonl");
            if let Some(sid) = parse_early_session_id(&output_path) {
                if let Some(event) = handler.on_session_id_discovered(issue_id, sid) {
                    let _ = tx.send(event).await;
                }
                session_id_sent = true;
            }
        }

        if is_alive(pid) {
            continue;
        }

        // プロセス終了を検出
        let meta = match read_meta(job_dir) {
            Ok(m) => m,
            Err(_) => break,
        };

        let output_path = job_dir.join("output.jsonl");
        let event = if let Some(result) = parse_output(&output_path) {
            handler.on_completed(result, &meta).await
        } else {
            handler.on_failed(issue_id, "process exited without result".to_string())
        };

        let _ = tx.send(event).await;
        break;
    }
}

/// 再起動時にジョブを復元する（全 workflow 共通）
pub async fn restore_jobs<W: WorkflowHandler>(
    handler: &Arc<W>,
    tx: &mpsc::Sender<W::Event>,
) -> Vec<JobMeta> {
    let jobs_dir = match ensure_strand_dir() {
        Ok(d) => d,
        Err(_) => return vec![],
    };

    let prefix = format!("{}-", handler.workflow_name());
    let entries = match fs::read_dir(&jobs_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let mut jobs = vec![];

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if !name.starts_with(&prefix) {
            continue;
        }

        let meta = match read_meta(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let pid = match read_pid(&path) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if is_alive(pid) {
            // まだ動いてる → monitor 起動
            spawn_monitor(
                Arc::clone(handler),
                path.clone(),
                meta.issue_id.clone(),
                tx.clone(),
            );

            jobs.push(meta);
        } else {
            // 死んでる → 結果を処理
            let output_path = path.join("output.jsonl");
            let completed = parse_output(&output_path).is_some();
            let event = if let Some(result) = parse_output(&output_path) {
                handler.on_completed(result, &meta).await
            } else {
                handler.on_failed(&meta.issue_id, "process exited without result".to_string())
            };
            let _ = tx.send(event).await;

            // 正常完了 → ログを残す、異常終了 → 削除
            if !completed {
                let _ = fs::remove_dir_all(&path);
            }
        }
    }

    jobs
}

/// job ディレクトリを削除する
pub fn cleanup_job(job_dir: &Path) {
    let _ = fs::remove_dir_all(job_dir);
}
