use std::collections::BTreeMap;

use crate::db::models::*;
use crate::logical_time::now_epoch_ms;

/// Deterministic LWW winner selector: higher `updated_at` wins;
/// ties break deterministically by lexicographically greater `device_id`.
fn lww_wins(a_ts: i64, a_dev: &str, b_ts: i64, b_dev: &str) -> bool {
    if a_ts != b_ts {
        a_ts > b_ts
    } else {
        a_dev >= b_dev
    }
}

fn merge_lww<T: Clone, K: Ord>(
    local: &[T],
    cloud: &[T],
    key_fn: impl Fn(&T) -> K,
    ts_fn: impl Fn(&T) -> i64,
    dev_fn: impl Fn(&T) -> &str,
) -> Vec<T> {
    let mut map: BTreeMap<K, T> = BTreeMap::new();
    for item in local {
        map.insert(key_fn(item), item.clone());
    }
    for c_item in cloud {
        let k = key_fn(c_item);
        match map.get(&k) {
            None => {
                map.insert(k, c_item.clone());
            }
            Some(l_item) => {
                if lww_wins(ts_fn(c_item), dev_fn(c_item), ts_fn(l_item), dev_fn(l_item)) {
                    map.insert(k, c_item.clone());
                }
            }
        }
    }
    map.into_values().collect()
}

fn merge_union<T: Clone, K: Ord>(
    local: &[T],
    cloud: &[T],
    key_fn: impl Fn(&T) -> K,
) -> Vec<T> {
    let mut map: BTreeMap<K, T> = BTreeMap::new();
    for item in local {
        map.insert(key_fn(item), item.clone());
    }
    for item in cloud {
        map.entry(key_fn(item)).or_insert_with(|| item.clone());
    }
    map.into_values().collect()
}

/// Deterministic Item-Level LWW + Append-Only Union Merge across devices.
pub fn merge_snapshots(local: &SyncSnapshot, cloud: &SyncSnapshot, device_id: &str) -> SyncSnapshot {
    SyncSnapshot {
        schema_version: 6,
        exported_at: now_epoch_ms(),
        device_id: device_id.to_string(),
        app_meta: merge_lww(&local.app_meta, &cloud.app_meta, |x| x.key.clone(), |x| x.updated_at, |x| &x.device_id),
        recurrence_rules: merge_lww(&local.recurrence_rules, &cloud.recurrence_rules, |x| x.id.clone(), |x| x.updated_at, |x| &x.device_id),
        tasks: merge_lww(&local.tasks, &cloud.tasks, |x| x.id.clone(), |x| x.updated_at, |x| &x.device_id),
        // Append-only audit logs never lose records from either device
        reschedule_events: merge_union(&local.reschedule_events, &cloud.reschedule_events, |x| x.id.clone()),
        cancellation_excuses: merge_union(&local.cancellation_excuses, &cloud.cancellation_excuses, |x| x.id.clone()),
        prayers_log: merge_lww(
            &local.prayers_log,
            &cloud.prayers_log,
            |x| (x.logical_date.clone(), x.prayer_key.clone()),
            |x| x.updated_at,
            |x| &x.device_id,
        ),
        day_summaries: merge_lww(&local.day_summaries, &cloud.day_summaries, |x| x.logical_date.clone(), |x| x.updated_at, |x| &x.device_id),
        focus_sessions: merge_lww(&local.focus_sessions, &cloud.focus_sessions, |x| x.id.clone(), |x| x.updated_at, |x| &x.device_id),
        distractions: merge_lww(&local.distractions, &cloud.distractions, |x| x.id.clone(), |x| x.updated_at, |x| &x.device_id),
        notes: merge_lww(&local.notes, &cloud.notes, |x| x.id.clone(), |x| x.updated_at, |x| &x.device_id),
        quotes: merge_lww(&local.quotes, &cloud.quotes, |x| x.id.clone(), |x| x.updated_at, |x| &x.device_id),
    }
}
