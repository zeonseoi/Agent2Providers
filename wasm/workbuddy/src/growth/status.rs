//! 成长状态四块独立只读；某块失败只返回该块错误，不把空值报成0余额。
use super::{list_tasks, school, spec, travel, GrowthAccount};
use agent2api_wasm_sdk::{json, Value};

fn row(task: &Value) -> Value {
    let code = task["task_code"].as_str().unwrap_or("");
    let spec = spec::spec_of(code);
    // 任务有效期（上游 `valid_start` / `valid_end`，ISO 8601 带时区串）。
    // 网关的「到期前自动领取」按 `validEnd` 判「今天 ≥ 到期日 − 1 天」，
    // 界面在任务行上显示「到期 YYYY-MM-DD」—— 缺这两个键时那两处都拿不到
    // 日期，功能会静默失效（没有报错，只是永远不命中）。
    json!({ "code": code, "title": task["title"], "description": task["description"], "status": task["accept_status"],
        "current": task["progress"]["current"], "target": task["progress"]["target"], "locked": task["locked"] == true,
        "unlockAt": task["unlock_at"],
        "validStart": task["valid_start"], "validEnd": task["valid_end"],
        "supported": spec.is_some(), "unforgeable": spec.as_ref().is_some_and(|s| s.unforgeable),
        "unforgeableReason": spec.as_ref().map(|s| s.reason).unwrap_or(""), "kind": spec.as_ref().map(|s| s.kind).unwrap_or(""), "source": spec.as_ref().map(|s| s.src).unwrap_or("") })
}
pub fn query(account: &GrowthAccount) -> Result<Value, String> {
    let tasks = match list_tasks(account, false) {
        Ok(tasks) => {
            let mut rows: Vec<_> = tasks.iter().map(row).collect();
            if let Ok(mp) = list_tasks(account, true) {
                for task in mp {
                    if rows.iter().any(|r| r["code"] == task["task_code"]) {
                        continue;
                    }
                    rows.push(row(&task));
                }
            }
            json!({ "tasks": rows, "error": null })
        }
        Err(error) => json!({ "tasks": [], "error": error }),
    };
    let energy = super::energy_balance(account);
    let streak = super::growth_streak(account);
    let travel = match travel::info(account) {
        Ok(Some(buddy)) => match travel::state(account) {
            Ok(state) => {
                json!({ "hasBuddy": true, "buddyName": buddy["name"], "state": state["state"], "dailyLimitReached": state["daily_limit_reached"], "recordId": state["record_id"], "rewardCredit": state["reward_credit"], "error": null })
            }
            Err(error) => json!({ "hasBuddy": true, "buddyName": buddy["name"], "error": error }),
        },
        Ok(None) => json!({ "hasBuddy": false, "buddyName": "", "state": "", "error": null }),
        Err(error) => json!({ "hasBuddy": null, "error": error }),
    };
    let school = match school::tasks(account) {
        Ok((tasks, active)) => {
            let chance = school::lottery(account).ok().map(|v| json!({ "balance": v["chance"]["balance"], "totalEarned": v["chance"]["total_earned"] }));
            let rows: Vec<_> = tasks.iter().map(|task| json!({ "code": task["task_code"], "title": task["title"], "status": task["status"], "current": task["progress"], "target": task["target_count"], "rewardCredit": task["reward_credit"] })).collect();
            json!({ "inPeriod": active, "tasks": rows, "lottery": chance, "error": null })
        }
        Err(error) => json!({ "inPeriod": null, "tasks": [], "lottery": null, "error": error }),
    };
    account.check()?;
    Ok(
        json!({ "id": account.session.record["id"], "uid": account.uid, "nick": account.nick, "energy": energy, "streak": streak, "tasks": tasks, "travel": travel, "school": school }),
    )
}

#[cfg(test)]
mod tests {
    /// 任务行必须透传有效期：网关的「到期前自动领取」与界面的「到期日期」
    /// 都读这两个键，缺了会静默失效。
    #[test]
    fn task_row_exposes_validity_window() {
        let row = super::row(&agent2api_wasm_sdk::json!({
            "task_code": "daily_chat", "title": "t", "description": "d",
            "accept_status": "accepted", "progress": {"current": 0, "target": 1},
            "locked": false, "unlock_at": null,
            "valid_start": "2026-11-01T00:00:00+08:00",
            "valid_end": "2026-11-02T23:59:00+08:00"
        }));
        assert_eq!(row["validEnd"], "2026-11-02T23:59:00+08:00");
        assert_eq!(row["validStart"], "2026-11-01T00:00:00+08:00");
    }
}
