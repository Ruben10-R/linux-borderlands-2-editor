//! Borderlands 2 experience-level table and XP<->level sync helpers.
//!
//! `XP_FOR_LEVEL[n-1]` is the minimum XP to be level `n` (levels 1..=80),
//! matching apocalyptech's proven table. Used by the Character tab's Sync.

pub const XP_FOR_LEVEL: [i64; 80] = [
    0, 358, 1241, 2850, 5376, 8997, 13886, 20208, 28126, 37798, 49377, 63016, 78861, 97061, 117757,
    141092, 167206, 196238, 228322, 263595, 302190, 344238, 389873, 439222, 492414, 549578, 610840,
    676325, 746158, 820463, 899363, 982980, 1071435, 1164850, 1263343, 1367034, 1476041, 1590483,
    1710476, 1836137, 1967582, 2104926, 2248285, 2397772, 2553501, 2715586, 2884139, 3059273,
    3241098, 3429728, 3625271, 3827840, 4037543, 4254491, 4478792, 4710556, 4949890, 5196902,
    5451701, 5714393, 5985086, 6263885, 6550897, 6846227, 7149982, 7462266, 7783184, 8112840,
    8451340, 8798786, 9155282, 9520931, 9895837, 10280103, 10673830, 11077120, 11490077, 11912801,
    12345393, 12787955,
];

/// Minimum XP required to be `level` (clamped to 1..=80).
pub fn xp_for_level(level: i64) -> i64 {
    let idx = (level.clamp(1, XP_FOR_LEVEL.len() as i64) - 1) as usize;
    XP_FOR_LEVEL[idx]
}

/// The highest level whose XP threshold is <= `xp` (1..=80).
pub fn level_for_xp(xp: i64) -> i64 {
    let mut level = 1i64;
    for (i, &need) in XP_FOR_LEVEL.iter().enumerate() {
        if xp >= need {
            level = (i + 1) as i64;
        } else {
            break;
        }
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_and_level_are_inverse() {
        for lvl in 1..=80 {
            assert_eq!(level_for_xp(xp_for_level(lvl)), lvl, "level {lvl}");
        }
        assert_eq!(xp_for_level(1), 0);
        assert_eq!(level_for_xp(0), 1);
        assert_eq!(level_for_xp(i64::MAX), 80);
        assert_eq!(xp_for_level(999), xp_for_level(80), "clamped above 80");
    }
}
