use anyhow::Result;
use serde::Deserialize;
use tokio::process::Command;

#[derive(Debug, Clone, Deserialize)]
pub struct Issue {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub priority: Option<u8>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub issue_type: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

pub fn short_id(id: &str) -> &str {
    id.rsplit_once('-').map(|(_, s)| s).unwrap_or(id)
}

fn bd_command(dir: Option<&str>) -> Command {
    let mut cmd = Command::new("bd");
    if let Some(d) = dir {
        cmd.arg("--db").arg(format!("{d}/.beads/beads.db"));
    }
    cmd
}

async fn run_bd(dir: Option<&str>, args: &[&str]) -> Result<Vec<u8>> {
    let output = bd_command(dir).args(args).output().await?;
    if !output.status.success() {
        anyhow::bail!(
            "bd {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

/// .beadsが存在しなければ bd init と bd setup claude を自動実行する
pub async fn check_init(dir: Option<&str>) -> Result<()> {
    let base = match dir {
        Some(d) => std::path::PathBuf::from(d),
        None => std::env::current_dir()?,
    };
    let beads_dir = base.join(".beads");
    if beads_dir.exists() {
        return Ok(());
    }

    eprintln!("Beads not initialized — running `bd init` and `bd setup claude`...");

    let init_output = Command::new("bd")
        .arg("init")
        .current_dir(&base)
        .output()
        .await?;
    if !init_output.status.success() {
        anyhow::bail!(
            "bd init failed: {}",
            String::from_utf8_lossy(&init_output.stderr)
        );
    }

    let setup_output = Command::new("bd")
        .args(["setup", "claude"])
        .current_dir(&base)
        .output()
        .await?;
    if !setup_output.status.success() {
        anyhow::bail!(
            "bd setup claude failed: {}",
            String::from_utf8_lossy(&setup_output.stderr)
        );
    }

    eprintln!("Beads initialized successfully.");
    Ok(())
}

pub async fn list_issues(dir: Option<&str>) -> Result<Vec<Issue>> {
    let stdout = run_bd(dir, ["list", "--json", "--limit", "0", "--all"].as_slice()).await?;
    let issues: Vec<Issue> = serde_json::from_slice(&stdout)?;
    let issues = issues
        .into_iter()
        .filter(|i| i.status != "closed" && i.issue_type.as_deref() == Some("epic"))
        .collect();
    Ok(issues)
}

pub async fn update_title(dir: Option<&str>, id: &str, title: &str) -> Result<()> {
    run_bd(dir, ["update", id, "--title", title].as_slice()).await?;
    Ok(())
}

pub async fn get_issue(dir: Option<&str>, id: &str) -> Result<Issue> {
    let stdout = run_bd(dir, ["show", id, "--json"].as_slice()).await?;
    let issues: Vec<Issue> = serde_json::from_slice(&stdout)?;
    issues
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("issue not found: {id}"))
}

pub async fn update_description(dir: Option<&str>, id: &str, description: &str) -> Result<()> {
    run_bd(dir, ["update", id, "--description", description].as_slice()).await?;
    Ok(())
}

pub async fn update_priority(dir: Option<&str>, id: &str, priority: u8) -> Result<()> {
    let p = priority.to_string();
    run_bd(dir, ["update", id, "--priority", &p].as_slice()).await?;
    Ok(())
}

pub async fn close_issue(dir: Option<&str>, id: &str) -> Result<()> {
    run_bd(dir, ["close", id].as_slice()).await?;
    Ok(())
}

pub async fn append_to_description(dir: Option<&str>, id: &str, content: &str) -> Result<()> {
    let issue = get_issue(dir, id).await?;
    let current = issue.description.unwrap_or_default();
    let new_desc = format!("{current}\n\n{content}");
    update_description(dir, id, &new_desc).await
}

pub async fn remove_label(dir: Option<&str>, id: &str, label: &str) -> Result<()> {
    run_bd(dir, ["label", "remove", id, label].as_slice()).await?;
    Ok(())
}

pub async fn add_label(dir: Option<&str>, id: &str, label: &str) -> Result<()> {
    run_bd(dir, ["label", "add", id, label].as_slice()).await?;
    Ok(())
}

/// Quick capture: epic, P2 で issue を作成し、ID を返す
pub async fn quick_create(dir: Option<&str>, title: &str) -> Result<String> {
    let stdout = run_bd(
        dir,
        ["q", title, "--type", "epic", "--priority", "2"].as_slice(),
    )
    .await?;
    Ok(String::from_utf8_lossy(&stdout).trim().to_string())
}
