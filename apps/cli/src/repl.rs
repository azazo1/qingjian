//! 交互模式: 拼音 -> 候选; 数字 -> 上屏; `:raw` 把上一次输入原样上屏 (相当于回车); `:up N` / `:down N` 调频; `:q` 退出.

use std::io::{self, BufRead, Write};

use qingjian_core::{Engine, Query};

use crate::display;
use crate::error::CliError;

pub fn run(engine: &mut Engine, limit: usize) -> Result<(), CliError> {
    eprintln!(
        "输入拼音查询候选, 输入序号上屏, :raw 原样上屏上一次输入 (回车), :del N 删掉第 N 个候选, :up N / :down N 调频 (升 / 降第 N 个候选), :q 退出."
    );
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut last: Option<Query> = None;
    loop {
        print!("> ");
        stdout.flush()?;
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == ":q" || line == ":quit" {
            break;
        }
        if let Ok(index) = line.parse::<usize>() {
            commit(engine, last.take(), index);
            continue;
        }
        if line == ":raw" {
            last = None;
            println!("原样上屏: {}", engine.take_raw());
            continue;
        }
        // 调频: `:up N` / `:down N` 把上一次查询的第 N 个 (从 1 数) 候选升 / 降一次, 等价于输入法里调频键 + K / J
        if let Some((up, index)) = line
            .strip_prefix(":up ")
            .map(|rest| (true, rest))
            .or_else(|| line.strip_prefix(":down ").map(|rest| (false, rest)))
            && let Ok(index) = index.trim().parse::<usize>()
        {
            let typed = last.as_ref().map(|q| q.text.clone());
            match last
                .as_ref()
                .and_then(|q| q.candidates.items.get(index.wrapping_sub(1)))
                .cloned()
            {
                Some(candidate) => {
                    let change = engine.adjust_frequency(&candidate, up);
                    println!("调频 {}: {change:?}", candidate.text);
                }
                None => println!("没有第 {index} 个候选"),
            }
            // 计数变了次序就会变, 重新查一遍给用户看
            if let Some(typed) = typed {
                last = display::show(engine, &typed, limit);
            }
            continue;
        }
        if let Some(index) = line
            .strip_prefix(":del ")
            .and_then(|rest| rest.trim().parse::<usize>().ok())
        {
            forget(engine, last.as_ref(), index);
            continue;
        }
        last = display::show(engine, line, limit);
    }
    Ok(())
}

/// 删掉上一次查询的第 `index` 个（从 1 数）候选：用户词整个删、词库词清学习。
fn forget(engine: &mut Engine, last: Option<&Query>, index: usize) {
    match last.and_then(|q| q.candidates.items.get(index.wrapping_sub(1))) {
        Some(candidate) => {
            let forgotten = engine.forget(candidate);
            println!("删除 {}: {forgotten:?}", candidate.text);
        }
        None => println!("没有第 {index} 个候选"),
    }
}

/// 把上一次查询的第 `index` 个（从 1 数）候选上屏。
fn commit(engine: &mut Engine, last: Option<Query>, index: usize) {
    match last.and_then(|q| q.candidates.items.into_iter().nth(index.wrapping_sub(1))) {
        Some(candidate) => {
            let text = engine.commit(&candidate);
            if text.is_empty() {
                // 这段拼音还没选完：上屏推迟到组句结束，选中的词先留在 Engine 里（能在 preedit 里看到、退格能拆回）
                println!(
                    "选中 {}（延迟上屏，还剩拼音 {}）",
                    candidate.text,
                    engine.composition().text()
                );
            } else {
                println!("上屏: {text}");
            }
        }
        None => println!("没有第 {index} 个候选"),
    }
}
