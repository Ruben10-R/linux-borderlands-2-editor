//! Character creation and group imports.
//!
//! [`SaveFile::new_character`] builds a fresh level-1 save whose protobuf is the
//! exact field set Gibbed's "New" produces (verified byte-for-byte by a golden
//! test against a real Gibbed New save). [`SaveFile::import_group`] copies a
//! group of fields (skills / missions / world / stats) from another save, the
//! way Gibbed's Import buttons do.

use crate::proto::{emit_varint_field, emit_wire2_field, parse_fields, push_varint};
use crate::{Result, SaveFile};

/// DLC character classes carry two extra flags that a base-game class omits.
/// Mechromancer (Gaige) = package 2, Psycho (Krieg) = package 3.
fn dlc_package_id(class_def: &str) -> Option<u64> {
    if class_def.contains("Mechromancer") {
        Some(2)
    } else if class_def.contains("Lilac") || class_def.contains("Psycho") {
        Some(3)
    } else {
        None
    }
}

impl SaveFile {
    /// Build a fresh level-1 character save for `class_def`, named `name`. A
    /// random 16-byte Save GUID is generated so every new save is unique.
    ///
    /// The output matches Gibbed's "New" byte-for-byte (see the golden test).
    pub fn new_character(class_def: &str, name: &str) -> Self {
        let mut guid = [0u8; 16];
        getrandom::getrandom(&mut guid).expect("system RNG unavailable");
        Self::new_character_with_guid(class_def, name, &guid)
    }

    /// Like [`Self::new_character`] but with a caller-supplied GUID — used by the
    /// golden test for a deterministic byte comparison.
    pub(crate) fn new_character_with_guid(class_def: &str, name: &str, guid: &[u8; 16]) -> Self {
        let mut p = Vec::new();
        emit_wire2_field(&mut p, 1, class_def.as_bytes()); // class
        emit_varint_field(&mut p, 2, 1); // level
        emit_varint_field(&mut p, 3, 0); // experience
        emit_varint_field(&mut p, 4, 0); // general_skill_points
        emit_varint_field(&mut p, 5, 0); // specialist_skill_points
        emit_wire2_field(&mut p, 6, &[0u8; 13]); // currency_on_hand (all zero)
        emit_varint_field(&mut p, 7, 0); // playthroughs_completed
        emit_wire2_field(&mut p, 13, &[0x08, 0, 0x10, 0, 0x18, 0]); // inventory_slots {0,0,0}

        // ui_preferences { 1: name, 2/3/4: zeroed sub-messages }.
        let mut ui = Vec::new();
        emit_wire2_field(&mut ui, 1, name.as_bytes());
        let z8 = [0x08, 0, 0x10, 0, 0x18, 0, 0x20, 0];
        emit_wire2_field(&mut ui, 2, &z8);
        emit_wire2_field(&mut ui, 3, &z8);
        emit_wire2_field(&mut ui, 4, &z8);
        emit_wire2_field(&mut p, 19, &ui);

        emit_varint_field(&mut p, 20, 1); // save_game_id (slot 1)
        emit_varint_field(&mut p, 21, 0); // plot_mission_number
        emit_varint_field(&mut p, 25, 0); // total_play_time
        emit_varint_field(&mut p, 31, 0); // is_badass_mode_save

        // save_guid: four little-endian fixed32 fields over the 16 GUID bytes.
        let mut g = Vec::new();
        for i in 0..4 {
            push_varint(&mut g, (((i + 1) as u64) << 3) | 5); // wire type 5 = fixed32
            g.extend_from_slice(&guid[i * 4..i * 4 + 4]);
        }
        emit_wire2_field(&mut p, 34, &g);

        // applied_customizations: head/skin "None", three empty slots between.
        for s in ["None", "", "", "", "None"] {
            emit_wire2_field(&mut p, 35, s.as_bytes());
        }

        emit_varint_field(&mut p, 37, 0); // active_mission_number
        emit_varint_field(&mut p, 42, 0); // num_challenge_prestiges
        if let Some(pkg) = dlc_package_id(class_def) {
            emit_varint_field(&mut p, 44, 1); // is_dlc_player_class
            emit_varint_field(&mut p, 45, pkg); // dlc_player_class_package_id
        }
        emit_varint_field(&mut p, 48, 0); // golden_keys_notified
        emit_varint_field(&mut p, 49, 0); // active_playthrough
        emit_varint_field(&mut p, 50, 0); // show_new_playthrough_notification
        emit_varint_field(&mut p, 51, 0); // received_default_weapon
        emit_varint_field(&mut p, 55, 0); // awesome_skill_disabled
        emit_varint_field(&mut p, 56, 0); // bank_size

        Self { proto: p }
    }

    /// Copy one [`ImportGroup`] from `source` into this save: drop this save's
    /// fields with those numbers, then append the source's (other fields kept
    /// verbatim, in order). Protobuf is read by field number, so order is safe.
    pub fn import_group(&mut self, source: &SaveFile, group: ImportGroup) -> Result<()> {
        let nums = group.field_numbers();
        let src_fields = parse_fields(&source.proto)?;
        let dst_fields = parse_fields(&self.proto)?;
        let mut out = Vec::with_capacity(self.proto.len());
        for f in &dst_fields {
            if !nums.contains(&f.number) {
                out.extend_from_slice(&self.proto[f.tag_start..f.end]);
            }
        }
        for f in &src_fields {
            if nums.contains(&f.number) {
                out.extend_from_slice(&source.proto[f.tag_start..f.end]);
            }
        }
        self.proto = out;
        Ok(())
    }
}

/// A named group of protobuf fields copied together by [`SaveFile::import_group`],
/// mirroring Gibbed's Import Skills / Missions / World / Stats.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ImportGroup {
    Skills,
    Missions,
    World,
    Stats,
}

impl ImportGroup {
    pub const ALL: [ImportGroup; 4] = [
        ImportGroup::Skills,
        ImportGroup::Missions,
        ImportGroup::World,
        ImportGroup::Stats,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ImportGroup::Skills => "Skills",
            ImportGroup::Missions => "Missions",
            ImportGroup::World => "World",
            ImportGroup::Stats => "Stats",
        }
    }

    /// A one-line description of what the group brings over.
    pub fn detail(self) -> &'static str {
        match self {
            ImportGroup::Skills => "skill tree + unspent skill points",
            ImportGroup::Missions => "mission progress (all playthroughs)",
            ImportGroup::World => "fast-travel unlocks, map discovery, playthrough",
            ImportGroup::Stats => "stats, challenges, and total play time",
        }
    }

    /// Top-level field numbers this group carries.
    fn field_numbers(self) -> &'static [u64] {
        match self {
            ImportGroup::Skills => &[4, 5, 8],
            ImportGroup::Missions => &[18, 21, 37],
            ImportGroup::World => &[7, 16, 17, 29, 30, 46, 49],
            ImportGroup::Stats => &[15, 25, 38, 39, 42],
        }
    }
}
