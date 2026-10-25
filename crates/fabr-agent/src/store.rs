use crate::now_utc;
use fabr_core::FabrError;
use fabr_core::taskrec::{TaskRecord, modify_record};
use std::path::Path;

fn take<T: PartialEq + Clone>(disk: &mut T, base: &T, mine: &T) {
    if mine != base {
        *disk = mine.clone();
    }
}

pub fn merge(disk: &mut TaskRecord, base: &TaskRecord, mine: &TaskRecord) {
    take(&mut disk.source, &base.source, &mine.source);
    take(&mut disk.title, &base.title, &mine.title);
    take(&mut disk.due, &base.due, &mine.due);
    take(&mut disk.pri, &base.pri, &mine.pri);
    take(&mut disk.state, &base.state, &mine.state);
    take(&mut disk.step, &base.step, &mine.step);
    take(&mut disk.waiting, &base.waiting, &mine.waiting);
    take(&mut disk.paused, &base.paused, &mine.paused);
    take(&mut disk.requests.delegate, &base.requests.delegate, &mine.requests.delegate);
    take(&mut disk.requests.cancel, &base.requests.cancel, &mine.requests.cancel);
    take(&mut disk.requests.redelegate, &base.requests.redelegate, &mine.requests.redelegate);
    take(&mut disk.grants.requested, &base.grants.requested, &mine.grants.requested);
    take(&mut disk.grants.granted, &base.grants.granted, &mine.grants.granted);
    take(&mut disk.plan_meta, &base.plan_meta, &mine.plan_meta);
    take(&mut disk.assertions, &base.assertions, &mine.assertions);
    take(&mut disk.approval, &base.approval, &mine.approval);
    take(&mut disk.verdict, &base.verdict, &mine.verdict);
    take(&mut disk.cost_usd, &base.cost_usd, &mine.cost_usd);
    take(&mut disk.links, &base.links, &mine.links);
    take(&mut disk.context, &base.context, &mine.context);
    take(&mut disk.spec, &base.spec, &mine.spec);
    take(&mut disk.success, &base.success, &mine.success);
    take(&mut disk.plan, &base.plan, &mine.plan);
    if mine.question != base.question {
        let answered_meanwhile = mine.question.is_none()
            && disk.question.as_ref().is_some_and(|q| q.answer.is_some())
            && base.question.as_ref().is_none_or(|q| q.answer.is_none());
        if !answered_meanwhile {
            disk.question = mine.question.clone();
        }
    }
    if mine.answers != base.answers {
        for a in &mine.answers {
            if !disk.answers.iter().any(|d| d.id == a.id) {
                disk.answers.push(a.clone());
            }
        }
    }
    if mine.log != base.log {
        match mine.log.strip_prefix(base.log.as_str()) {
            Some(suffix) => {
                if !disk.log.is_empty() && !disk.log.ends_with('\n') && !suffix.starts_with('\n') {
                    disk.log.push('\n');
                }
                disk.log.push_str(suffix);
            }
            None => disk.log = mine.log.clone(),
        }
    }
}

pub fn update(path: &Path, mine: &mut TaskRecord, base: &mut TaskRecord) -> Result<(), FabrError> {
    let (merged, ()) = modify_record(path, &now_utc(), |disk| merge(disk, base, mine))?;
    let merged = fabr_core::taskrec::parse(&fabr_core::taskrec::render(&merged))?;
    *base = merged.clone();
    *mine = merged;
    Ok(())
}

pub fn mutate<T>(path: &Path, f: impl FnOnce(&mut TaskRecord) -> T) -> Result<(TaskRecord, T), FabrError> {
    modify_record(path, &now_utc(), f)
}
