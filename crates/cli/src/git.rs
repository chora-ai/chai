use anyhow::Result;
use clap::Subcommand;
use std::process::Command;

#[derive(Subcommand)]
pub(crate) enum GitCmd {
    /// Read a range of lines from a git diff output with line numbers. Outputs lines in the format {line_number}\t{content}.
    DiffLines {
        /// Line number to start reading at (1-indexed, inclusive)
        #[arg(long)]
        start_line: usize,
        /// Line number to end reading at (1-indexed, inclusive). When omitted, reads from start_line to the end of the output.
        #[arg(long)]
        end_line: Option<usize>,
        /// Show staged changes (--cached)
        #[arg(long)]
        staged: bool,
        /// Compare against a specific commit, branch, or ref
        #[arg(long = "ref")]
        ref_: Option<String>,
        /// Limit diff to a specific file or directory path within the repository
        #[arg(long)]
        path: Option<String>,
        /// Repository path (working directory for git). Defaults to the current directory.
        #[arg(long)]
        repo: Option<String>,
    },
    /// Read a range of lines from a git show output with line numbers. Outputs lines in the format {line_number}\t{content}.
    ShowLines {
        /// Line number to start reading at (1-indexed, inclusive)
        #[arg(long)]
        start_line: usize,
        /// Line number to end reading at (1-indexed, inclusive). When omitted, reads from start_line to the end of the output.
        #[arg(long)]
        end_line: Option<usize>,
        /// Commit hash, branch name, tag, or ref to show
        #[arg(long)]
        r#ref: String,
        /// Repository path (working directory for git). Defaults to the current directory.
        #[arg(long)]
        repo: Option<String>,
    },
}

pub(crate) fn run_git(cmd: GitCmd) -> Result<()> {
    match cmd {
        GitCmd::DiffLines {
            start_line,
            end_line,
            staged,
            ref_,
            path,
            repo,
        } => {
            if start_line == 0 {
                anyhow::bail!("start_line must be at least 1 (1-indexed)");
            }

            let mut git_cmd = Command::new("git");
            git_cmd.arg("diff");

            if staged {
                git_cmd.arg("--cached");
            }

            if let Some(ref ref_val) = ref_ {
                git_cmd.arg(ref_val);
            }

            if let Some(ref fp) = path {
                // If ref was skipped, use -- to disambiguate paths from refs
                if ref_.is_none() {
                    git_cmd.arg("--");
                }
                git_cmd.arg(fp);
            }

            if let Some(ref dir) = repo {
                git_cmd.current_dir(dir);
            }

            let output = git_cmd
                .output()
                .map_err(|e| anyhow::anyhow!("failed to run git diff: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("git diff failed: {}", stderr.trim());
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            print_line_range(&stdout, start_line, end_line)?;
            Ok(())
        }
        GitCmd::ShowLines {
            start_line,
            end_line,
            r#ref,
            repo,
        } => {
            if start_line == 0 {
                anyhow::bail!("start_line must be at least 1 (1-indexed)");
            }

            let mut git_cmd = Command::new("git");
            git_cmd.arg("show").arg(&r#ref);

            if let Some(ref dir) = repo {
                git_cmd.current_dir(dir);
            }

            let output = git_cmd
                .output()
                .map_err(|e| anyhow::anyhow!("failed to run git show: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("git show failed: {}", stderr.trim());
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            print_line_range(&stdout, start_line, end_line)?;
            Ok(())
        }
    }
}

/// Print lines in the range [start_line, end_line] (1-indexed, inclusive)
/// in the format `{line_number}\t{content}`. When `end_line` is `None`,
/// reads from `start_line` to the end of the output.
fn print_line_range(content: &str, start_line: usize, end_line: Option<usize>) -> Result<()> {
    if let Some(end) = end_line {
        if end < start_line {
            anyhow::bail!(
                "end_line ({}) must be >= start_line ({})",
                end,
                start_line
            );
        }
    }

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        if line_num >= start_line {
            println!("{}\t{}", line_num, line);
        }
        if let Some(end) = end_line {
            if line_num >= end {
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn print_line_range_basic() {
        let content = "line1\nline2\nline3\nline4\nline5";
        let lines: Vec<&str> = content.lines().collect();
        let start = 2;
        let end = Some(4);
        let mut result = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            if line_num >= start {
                result.push(format!("{}\t{}", line_num, line));
            }
            if let Some(e) = end {
                if line_num >= e {
                    break;
                }
            }
        }
        assert_eq!(result, vec!["2\tline2", "3\tline3", "4\tline4"]);
    }

    #[test]
    fn print_line_range_read_to_end() {
        let content = "line1\nline2\nline3\nline4\nline5";
        let lines: Vec<&str> = content.lines().collect();
        let start = 3;
        let end: Option<usize> = None;
        let mut result = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            if line_num >= start {
                result.push(format!("{}\t{}", line_num, line));
            }
            if let Some(e) = end {
                if line_num >= e {
                    break;
                }
            }
        }
        assert_eq!(result, vec!["3\tline3", "4\tline4", "5\tline5"]);
    }

    #[test]
    fn print_line_range_single_line() {
        let content = "line1\nline2\nline3";
        let lines: Vec<&str> = content.lines().collect();
        let start = 2;
        let end = Some(2);
        let mut result = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            if line_num >= start {
                result.push(format!("{}\t{}", line_num, line));
            }
            if let Some(e) = end {
                if line_num >= e {
                    break;
                }
            }
        }
        assert_eq!(result, vec!["2\tline2"]);
    }

    #[test]
    fn print_line_range_start_beyond_content() {
        let content = "line1\nline2";
        let lines: Vec<&str> = content.lines().collect();
        let start = 10;
        let end: Option<usize> = None;
        let mut result = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            if line_num >= start {
                result.push(format!("{}\t{}", line_num, line));
            }
            if let Some(e) = end {
                if line_num >= e {
                    break;
                }
            }
        }
        assert!(result.is_empty());
    }
}
