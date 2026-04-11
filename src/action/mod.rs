pub mod navigate;
pub mod state;

use crate::core::ConfirmAction;

/// strand 上の全操作を表現するデータ型。
/// キーハンドラはこれを返すだけ。実行は App::process_action() が行う。
#[derive(Debug, Clone)]
#[allow(dead_code)] // variant は段階的に使用される
pub enum AppAction {
    // ── Navigation ──
    Next,
    Previous,
    OpenDetail(String),
    OpenChildDetail(String),
    Back,
    NavigateIssue {
        forward: bool,
    },

    // ── KeyBar（セレクタ・確認） ──
    OpenSelector(SelectorDef),
    OpenConfirm(ConfirmAction),
    CloseKeyBar,
    SyncFilter,
    Confirm(ConfirmAction),

    // ── AI workflows ──
    StartEnrich(String),
    StartImplement {
        issue_id: String,
        epic_id: Option<String>,
    },
    StartSplit(String),

    // ── Impl operations ──
    MergeImpl(String),
    DiscardImpl(String),
    RetryImpl(String),
    MergeEpic(String),

    // ── State changes ──
    SetStatus {
        issue_id: String,
        status: String,
    },
    SetPriority {
        issue_id: String,
        priority: u8,
    },

    // ── Editor ──
    QuickCreate,
    EditDescription(String),

    // ── Clipboard ──
    CopyId(String),
    CopyResumeCommand(String),
    CopyLogCommand(String),
    CopyWorktreePath(String),

    // ── Filter ──
    ClearFilter,
    OpenFilterStatusToggle,
    OpenFilterLabelToggle,

    // ── System ──
    Notify(String),
    ReloadIssues,
}

/// Selector の定義。UI の表示内容と、各選択肢に対応する AppAction を一緒に持つ。
/// Selector 自身は AppAction の中身を知らない — 選ばれたら返すだけ。
#[derive(Debug, Clone)]
pub struct SelectorDef {
    pub items: Vec<SelectorItem>,
    pub initial_cursor: usize,
}

#[derive(Debug, Clone)]
pub struct SelectorItem {
    pub shortcut: String,
    pub label: String,
    pub action: AppAction,
}
