//! 记录未被现有规则（状态码/关键词）拦截的失败错误原文，供人工或 AI 分析后
//! 增补 disable_keywords。状态码一并记录，便于判断哪类失败值得重试。
//!
//! 输出文件：<data_dir>/disable-keyword-candidates.txt，每行 `{status} {归一化原文}`。
//! 按 status + 归一化原文去重；写入是 best-effort，失败仅告警，绝不影响转发主流程。

use std::collections::HashSet;

const LOG_FILE_NAME: &str = "disable-keyword-candidates.txt";

/// 归一化错误原文用于去重与单行存储：换行/制表转空格、压缩连续空白、去首尾空白、转小写。
fn normalize_message(message: &str) -> String {
    let mut result = String::with_capacity(message.len());
    let mut last_was_space = false;
    for ch in message.chars() {
        let c = if ch.is_whitespace() { ' ' } else { ch };
        if c == ' ' {
            if last_was_space {
                continue;
            }
            last_was_space = true;
        } else {
            last_was_space = false;
        }
        result.push(c);
    }
    result.trim().to_lowercase()
}

/// 记录一条未知失败（状态码 + 归一化原文）。已存在的相同行会被跳过，保证文件无重复。
pub async fn record_unknown_failure(status: u16, message: &str) {
    let normalized = normalize_message(message);
    if normalized.is_empty() {
        return;
    }
    let line = format!("{status} {normalized}");

    if let Err(e) = append_if_absent(&line).await {
        log::warn!("Failed to record disable-keyword candidate: {e}");
    }
}

async fn append_if_absent(line: &str) -> Result<(), String> {
    let dir = crate::data_dir::resolve_data_dir().map_err(|e| e.to_string())?;
    let path = dir.join(LOG_FILE_NAME);

    let existing = match tokio::fs::read_to_string(&path).await {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };

    let seen: HashSet<&str> = existing.lines().collect();
    if seen.contains(line) {
        return Ok(());
    }

    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    file.write_all(line.as_bytes())
        .await
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    file.write_all(b"\n")
        .await
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_whitespace_and_lowercases() {
        let input = "  Your   credit\n balance\tis  too LOW  ";
        assert_eq!(normalize_message(input), "your credit balance is too low");
    }

    #[test]
    fn normalize_empty_returns_empty() {
        assert_eq!(normalize_message("   \n\t  "), "");
    }
}
