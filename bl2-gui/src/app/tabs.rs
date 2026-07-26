//! Per-tab render functions (Character/General/Currency/Items/Fast Travel/
//! Vehicle/Raw/About) + the profile view and the item detail form. Called from
//! `App::ui`. Free functions taking `(&mut Doc, &mut Ui, accent)`.

use eframe::egui;

use super::*;
use crate::theme;

/// The account-profile view (profile.bin): Golden Keys (editable) + Badass (read).
pub(super) fn profile_view(pdoc: &mut ProfileDoc, ui: &mut egui::Ui, accent: egui::Color32) {
    ui.horizontal(|ui| {
        theme::coin(ui, 20.0);
        ui.label(
            egui::RichText::new("Account Profile")
                .color(accent)
                .size(18.0)
                .strong(),
        );
    });
    ui.add_space(4.0);
    egui::Grid::new("profile")
        .num_columns(2)
        .spacing([24.0, 8.0])
        .striped(true)
        .show(ui, |ui| {
            field(ui, "File", &pdoc.name, accent);

            key(ui, "Golden Keys", accent);
            ui.horizontal(|ui| {
                theme::coin(ui, 16.0);
                edit_number(ui, &mut pdoc.golden_keys, 1.0);
                pdoc.golden_keys = pdoc.golden_keys.clamp(0, 255);
                ui.weak("0–255");
            });
            ui.end_row();

            key(ui, "Badass Rank", accent);
            ui.horizontal(|ui| {
                theme::flag(ui, 16.0, accent);
                edit_number(ui, &mut pdoc.badass_rank, 100.0);
                pdoc.badass_rank = pdoc.badass_rank.clamp(0, MAX);
            });
            ui.end_row();

            key(ui, "Badass Tokens (unspent)", accent);
            ui.label(pdoc.badass_tokens.to_string());
            ui.end_row();

            if let Some((unlocked, total)) = pdoc.profile.customization_stats() {
                key(ui, "Customizations", accent);
                ui.horizontal(|ui| {
                    theme::head(ui, 16.0, accent);
                    let shown = match pdoc.pending_customizations {
                        Some(true) => format!("{total} / {total} (unlock all pending)"),
                        Some(false) => format!("0 / {total} (lock all pending)"),
                        None => format!("{unlocked} / {total} unlocked"),
                    };
                    ui.label(shown);
                    if ui.button("Unlock all").clicked() {
                        pdoc.pending_customizations = Some(true);
                    }
                    if ui.button("Lock all").clicked() {
                        pdoc.pending_customizations = Some(false);
                    }
                });
                ui.end_row();
            }
        });
    ui.add_space(6.0);
    ui.weak("Changes apply on Save/Download. Raising Badass Rank grants the extra tokens to spend. \u{201c}Unlock all\u{201d} unlocks every head, skin and vehicle skin.");
    ui.add_space(2.0);
    ui.colored_label(
        theme::DANGER,
        "⚠ Back up profile.bin first. With Steam Cloud on it may sync — verify in-game.",
    );
}

/// Character tab: file, name, class, level, XP, skill points.
pub(super) fn character_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    egui::Grid::new("character")
        .num_columns(2)
        .spacing([24.0, 8.0])
        .striped(true)
        .show(ui, |ui| {
            field(ui, "File", &doc.name, accent);

            key(ui, "Name", accent);
            ui.text_edit_singleline(&mut doc.char_name);
            ui.end_row();

            key(ui, "Class", accent);
            let current = bl2_save::CLASSES
                .iter()
                .find(|(_, path)| *path == doc.class_def)
                .map(|(disp, _)| *disp)
                .unwrap_or("(unknown)");
            egui::ComboBox::from_id_salt("class_combo")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (disp, path) in bl2_save::CLASSES {
                        if ui.selectable_label(doc.class_def == path, disp).clicked() {
                            doc.class_def = path.to_string();
                        }
                    }
                });
            ui.end_row();

            key(ui, "Head", accent);
            customization_combo(ui, "head_combo", &doc.class_def, true, &mut doc.head);
            ui.end_row();
            key(ui, "Skin", accent);
            customization_combo(ui, "skin_combo", &doc.class_def, false, &mut doc.skin);
            ui.end_row();

            key(ui, "Level", accent);
            ui.horizontal(|ui| {
                edit_number(ui, &mut doc.level, 1.0);
                if ui
                    .button("Sync")
                    .on_hover_text("Set XP to the minimum for this level")
                    .clicked()
                {
                    doc.xp = bl2_save::xp_for_level(doc.level);
                }
            });
            ui.end_row();
            key(ui, "XP", accent);
            ui.horizontal(|ui| {
                edit_number(ui, &mut doc.xp, 1000.0);
                if ui
                    .button("Sync")
                    .on_hover_text("Set level to match this XP")
                    .clicked()
                {
                    doc.level = bl2_save::level_for_xp(doc.xp);
                }
            });
            ui.end_row();
            key(ui, "Skill Points", accent);
            edit_number(ui, &mut doc.skill_points, 1.0);
            ui.end_row();
            key(ui, "Specialist Points", accent);
            edit_number(ui, &mut doc.specialist_skill_points, 1.0);
            ui.end_row();
        });
    ui.add_space(4.0);
    ui.weak("Changing class does not reset skills — level/skill mismatch may look odd in-game. \u{201c}Sync\u{201d} keeps Level and XP consistent.");
}

/// A head/skin picker for the character's class. Writes the chosen asset path
/// (or the stock "Default") into `current`. Head/skin apply on Save/Download.
pub(super) fn customization_combo(
    ui: &mut egui::Ui,
    id_salt: &str,
    class_def: &str,
    is_head: bool,
    current: &mut String,
) {
    let display = if current == "0" || current.is_empty() || current.contains("_Default") {
        "Default".to_string()
    } else {
        bl2_save::customization_name(current)
            .map(str::to_string)
            .unwrap_or_else(|| current.rsplit('.').next().unwrap_or(current).to_string())
    };
    egui::ComboBox::from_id_salt(id_salt)
        .width(320.0)
        .selected_text(display)
        .show_ui(ui, |ui| {
            if let Some(def) = bl2_save::default_customization(class_def, is_head) {
                if ui.selectable_label(*current == def, "Default").clicked() {
                    *current = def;
                }
            }
            for c in bl2_save::customizations(class_def, is_head) {
                if ui.selectable_label(*current == c.path, &c.name).clicked() {
                    *current = c.path;
                }
            }
        });
}

/// General tab: playthrough progression + read-only save info.
pub(super) fn general_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    const PT: [&str; 3] = [
        "Normal (NVHM)",
        "True Vault Hunter (TVHM)",
        "Ultimate Vault Hunter (UVHM)",
    ];

    ui.horizontal(|ui| {
        theme::flag(ui, 20.0, accent);
        ui.label(
            egui::RichText::new("Playthrough")
                .color(accent)
                .size(18.0)
                .strong(),
        );
    });
    ui.add_space(4.0);

    egui::Grid::new("general")
        .num_columns(2)
        .spacing([24.0, 8.0])
        .striped(true)
        .show(ui, |ui| {
            key(ui, "Playthroughs completed", accent);
            ui.horizontal(|ui| {
                edit_number(ui, &mut doc.playthroughs_completed, 1.0);
                doc.playthroughs_completed = doc.playthroughs_completed.clamp(0, 3);
                let hint = match doc.playthroughs_completed {
                    0 => "TVHM & UVHM locked",
                    1 => "TVHM unlocked",
                    _ => "TVHM & UVHM unlocked",
                };
                ui.weak(hint);
            });
            ui.end_row();

            key(ui, "Current playthrough", accent);
            let cur = *PT
                .get(doc.active_playthrough.clamp(0, 2) as usize)
                .unwrap_or(&PT[0]);
            egui::ComboBox::from_id_salt("playthrough_combo")
                .selected_text(cur)
                .show_ui(ui, |ui| {
                    for (i, name) in PT.iter().enumerate() {
                        if ui
                            .selectable_label(doc.active_playthrough == i as i64, *name)
                            .clicked()
                        {
                            doc.active_playthrough = i as i64;
                        }
                    }
                });
            ui.end_row();

            key(ui, "Overpower level", accent);
            ui.horizontal(|ui| {
                edit_number(ui, &mut doc.op_level, 1.0);
                doc.op_level = doc.op_level.clamp(0, MAX_OP_LEVEL);
                ui.weak(if doc.op_level == 0 {
                    "off"
                } else {
                    "OP levels unlocked"
                });
                ui.weak(format!("0–{MAX_OP_LEVEL}"));
            });
            ui.end_row();

            key(ui, "Backpack slots", accent);
            ui.horizontal(|ui| {
                edit_number(ui, &mut doc.backpack_size, 3.0);
                doc.backpack_size = doc.backpack_size.clamp(12, 39);
                ui.weak("12–39 (snaps to +3 per SDU)");
            });
            ui.end_row();

            key(ui, "Bank slots", accent);
            ui.horizontal(|ui| {
                edit_number(ui, &mut doc.bank_size, 2.0);
                doc.bank_size = doc.bank_size.clamp(6, 200);
                ui.weak("snaps to +2 per SDU");
            });
            ui.end_row();

            if let Some(id) = doc.save.save_game_id() {
                field(ui, "Save ID", &id.to_string(), accent);
            }
            if let Some(secs) = doc.save.time_played() {
                let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
                field(ui, "Time played", &format!("{h}h {m:02}m {s:02}s"), accent);
            }
        });

    ui.add_space(6.0);
    ui.colored_label(
        theme::DANGER,
        "⚠ Set \u{201c}playthroughs completed\u{201d} to unlock TVHM/UVHM. Changing the current \
         playthrough switches the mode you load into — only do so if that mode is unlocked.",
    );
    ui.weak(
        "Overpower level only matters at level 72 with UVHM completed; after saving, pick the OP \
         level at the character-select screen.",
    );
}

/// Currency tab: money, eridium, seraph crystals and torgue tokens.
pub(super) fn currency_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    egui::Grid::new("currency")
        .num_columns(2)
        .spacing([24.0, 8.0])
        .striped(true)
        .show(ui, |ui| {
            key(ui, "Money", accent);
            ui.horizontal(|ui| {
                theme::coin(ui, 16.0);
                edit_number(ui, &mut doc.money, 5000.0);
            });
            ui.end_row();
            key(ui, "Eridium", accent);
            ui.horizontal(|ui| {
                theme::eridium(ui, 16.0);
                edit_number(ui, &mut doc.eridium, 1.0);
            });
            ui.end_row();
            key(ui, "Seraph Crystals", accent);
            ui.horizontal(|ui| {
                theme::seraph(ui, 16.0);
                edit_number(ui, &mut doc.seraph, 1.0);
            });
            ui.end_row();
            key(ui, "Torgue Tokens", accent);
            ui.horizontal(|ui| {
                theme::torgue(ui, 16.0);
                edit_number(ui, &mut doc.torgue, 1.0);
            });
            ui.end_row();
        });
}

/// Vehicle tab: equip skins for each vehicle family (two loadout slots).
pub(super) fn vehicle_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    ui.horizontal(|ui| {
        theme::wheel(ui, 20.0, accent);
        ui.label(
            egui::RichText::new("Vehicle skins")
                .color(accent)
                .size(18.0)
                .strong(),
        );
    });
    ui.add_space(4.0);
    egui::Grid::new("vehicle")
        .num_columns(3)
        .spacing([16.0, 8.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("");
            ui.weak("Slot 1");
            ui.weak("Slot 2");
            ui.end_row();
            for (i, fam) in bl2_save::VEHICLE_FAMILIES.iter().enumerate() {
                key(ui, fam.name, accent);
                vehicle_skin_combo(ui, &format!("veh_{i}_1"), fam.token, &mut doc.vehicle[i].0);
                vehicle_skin_combo(ui, &format!("veh_{i}_2"), fam.token, &mut doc.vehicle[i].1);
                ui.end_row();
            }
        });
    ui.add_space(6.0);
    ui.weak(
        "Two skin slots per vehicle; applies on Save/Download. To unlock every vehicle skin first, \
         use the profile's \u{201c}Unlock all customizations\u{201d}.",
    );
}

/// A vehicle-skin picker for one family + slot. Writes the chosen path (or "None").
pub(super) fn vehicle_skin_combo(
    ui: &mut egui::Ui,
    id_salt: &str,
    token: &str,
    current: &mut String,
) {
    let display = if current.is_empty() || current == "None" {
        "None".to_string()
    } else {
        bl2_save::vehicle_skin_name(current)
            .map(str::to_string)
            .unwrap_or_else(|| current.rsplit('.').next().unwrap_or(current).to_string())
    };
    egui::ComboBox::from_id_salt(id_salt)
        .width(220.0)
        .selected_text(display)
        .show_ui(ui, |ui| {
            let none = current.is_empty() || current == "None";
            if ui.selectable_label(none, "None").clicked() {
                *current = "None".to_string();
            }
            for sk in bl2_save::vehicle_skins(token) {
                if ui.selectable_label(*current == sk.path, &sk.name).clicked() {
                    *current = sk.path.clone();
                }
            }
        });
}

/// Items tab: backpack + bank list with per-item level + parts editing.
pub(super) fn items_tab(
    doc: &mut Doc,
    ui: &mut egui::Ui,
    accent: egui::Color32,
    text: egui::Color32,
) -> Option<(bool, String)> {
    let mut status = None;

    // Import: paste one OR MANY BL2(...) codes (any separators) → backpack/bank.
    ui.horizontal(|ui| {
        theme::crate_icon(ui, 14.0, accent);
        ui.label("Add item code(s):");
        ui.add(
            egui::TextEdit::singleline(&mut doc.import_code)
                .hint_text("paste one or more BL2(...) codes")
                .desired_width(300.0),
        );
        let found = bl2_save::extract_codes(&doc.import_code).len();
        let mut do_import = None;
        if ui
            .add_enabled(found > 0, egui::Button::new("→ Backpack"))
            .clicked()
        {
            do_import = Some(false);
        }
        if ui
            .add_enabled(found > 0, egui::Button::new("→ Bank"))
            .clicked()
        {
            do_import = Some(true);
        }
        if found > 1 {
            ui.weak(format!("{found} codes"));
        }
        if ui
            .button("📚 Code library")
            .on_hover_text("Browse a library of item codes to add")
            .clicked()
        {
            doc.show_library = true;
        }
        if let Some(to_bank) = do_import {
            let (ok, failed) = doc.save.add_items_from_codes(&doc.import_code, to_bank);
            rebuild_items_preserving_levels(doc);
            doc.import_code.clear();
            let where_ = if to_bank { "bank" } else { "backpack" };
            status = Some(if failed == 0 {
                (false, format!("Imported {ok} item(s) into {where_}."))
            } else {
                (
                    true,
                    format!("Imported {ok} into {where_}; {failed} code(s) failed."),
                )
            });
        }
    });
    ui.add_space(4.0);

    if doc.items.is_empty() {
        ui.weak("No items in backpack or bank. Paste a BL2(...) code above to add one.");
        return status;
    }
    // Convenience: set every levelable item to one level.
    ui.horizontal(|ui| {
        ui.label("Set all to level");
        ui.add(egui::DragValue::new(&mut doc.item_level).speed(1.0));
        doc.item_level = doc.item_level.clamp(1, 127);
        if ui
            .button("Apply to all")
            .on_hover_text("Sets every levelable item (both bags) to this level. Locked ⚠ items are left alone.")
            .clicked()
        {
            let lvl = doc.item_level;
            for v in doc.items.iter_mut().filter(|v| v.levelable) {
                v.level = lvl;
            }
        }
        let char_level = doc.level.clamp(1, 127);
        if ui
            .button(format!("Sync to character level ({char_level})"))
            .on_hover_text("Levels every item (both bags) to your character's level. Locked ⚠ items are left alone.")
            .clicked()
        {
            doc.item_level = char_level;
            for v in doc.items.iter_mut().filter(|v| v.levelable) {
                v.level = char_level;
            }
        }
    });
    ui.add_space(4.0);

    // Backpack / Bank split — pick which bag to show.
    let backpack_n = doc.items.iter().filter(|v| !v.is_bank).count();
    let bank_n = doc.items.iter().filter(|v| v.is_bank).count();
    ui.horizontal(|ui| {
        theme::crate_icon(ui, 14.0, if doc.show_bank { text } else { accent });
        if ui
            .selectable_label(!doc.show_bank, format!("Backpack ({backpack_n})"))
            .clicked()
        {
            doc.show_bank = false;
        }
        ui.add_space(8.0);
        theme::crate_icon(ui, 14.0, if doc.show_bank { accent } else { text });
        if ui
            .selectable_label(doc.show_bank, format!("Bank ({bank_n})"))
            .clicked()
        {
            doc.show_bank = true;
        }
    });
    ui.add_space(2.0);

    let show_bank = doc.show_bank;
    if doc.items.iter().filter(|v| v.is_bank == show_bank).count() == 0 {
        ui.weak(if show_bank {
            "Bank is empty."
        } else {
            "Backpack is empty."
        });
    }

    let mut open_parts = None;
    let mut copy_code = None;
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        egui::Grid::new("items_grid")
            .num_columns(5)
            .spacing([22.0, 6.0])
            .min_col_width(56.0)
            .striped(true)
            .show(ui, |ui| {
                for h in ["Kind", "Level", "Name", "", ""] {
                    ui.label(egui::RichText::new(h).color(accent).strong().size(13.0));
                }
                ui.end_row();

                for v in doc.items.iter_mut().filter(|v| v.is_bank == show_bank) {
                    let (kcol, kind) = if v.is_weapon { (accent, "weapon") } else { (text, "item") };
                    ui.label(egui::RichText::new(kind).color(kcol).strong());
                    if v.levelable {
                        ui.add(egui::DragValue::new(&mut v.level).speed(1.0));
                        v.level = v.level.clamp(1, 127);
                    } else {
                        ui.horizontal(|ui| {
                            ui.monospace(format!("Lv {}", v.level));
                            ui.label(egui::RichText::new("⚠").color(theme::DANGER)).on_hover_text(
                                "No-level item (starter/special gear — some grenades, relics, class mods). \
                                 Leveling it can make the game drop it, so it's locked.",
                            );
                        });
                    }
                    // Give the name column a bit of fixed room so the table is
                    // comfortably wide (not hugging the text).
                    ui.add_sized(
                        [220.0, 18.0],
                        egui::Label::new(egui::RichText::new(&v.display).strong()).truncate(),
                    )
                    .on_hover_text(format!("{}\n{}", v.name, v.details));
                    if ui.small_button("Parts").clicked() {
                        open_parts = Some(v.id);
                    }
                    if ui
                        .small_button("Code")
                        .on_hover_text("Copy this item's shareable BL2(...) code to the clipboard.")
                        .clicked()
                    {
                        copy_code = Some(v.id);
                    }
                    ui.end_row();
                }
            });
    });
    if let Some(oid) = open_parts {
        doc.editing_parts = if doc.editing_parts == Some(oid) {
            None
        } else {
            Some(oid)
        };
        doc.editing_part_slot = None;
        doc.part_filter.clear();
    }
    if let Some(id) = copy_code {
        status = Some(match doc.save.item_code(id) {
            Ok(Some(code)) => {
                ui.ctx().copy_text(code.clone());
                (false, format!("Copied code to clipboard: {code}"))
            }
            Ok(None) => (true, "Could not build a code for that item.".to_string()),
            Err(e) => (true, format!("Code failed: {e}")),
        });
    }
    ui.add_space(2.0);
    ui.weak(
        "Edit a level or use \u{201c}Apply to all\u{201d}, then Save/Download. Locked \u{26a0} items can't be leveled. \
         \u{201c}Parts\u{201d} swaps parts; \u{201c}Code\u{201d} copies a shareable BL2(...) code. Edited items unequip in-game — re-equip.",
    );
    parts_editor(doc, ui, accent);
    status
}

/// Fast Travel tab: tick which stations are unlocked (base game + DLC), grouped
/// by pack. Changes are scratch and written to field 16 on Save/Download.
pub(super) fn fast_travel_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    let catalog = bl2_save::stations_catalog();
    let last = doc.save.last_station();

    ui.horizontal(|ui| {
        theme::signpost(ui, 20.0, accent);
        ui.label(
            egui::RichText::new("Fast Travel")
                .color(accent)
                .size(18.0)
                .strong(),
        );
    });
    if let Some(l) = &last {
        let shown = bl2_save::station_display_name(l).unwrap_or(l);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Current station:").color(accent));
            ui.label(shown);
        });
    }
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        if ui.button("Unlock all").clicked() {
            for s in catalog {
                doc.unlocked.insert(s.rn.clone());
            }
        }
        if ui.button("Lock all").clicked() {
            for s in catalog {
                doc.unlocked.remove(&s.rn);
            }
        }
        let on = catalog
            .iter()
            .filter(|s| doc.unlocked.contains(&s.rn))
            .count();
        ui.label(egui::RichText::new(format!("{on} / {} unlocked", catalog.len())).color(accent));
    });
    ui.add_space(2.0);

    egui::ScrollArea::vertical()
        .max_height(340.0)
        .show(ui, |ui| {
            let mut pack = "";
            for s in catalog {
                if s.pack != pack {
                    pack = &s.pack;
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(pack).color(accent).strong());
                }
                let mut on = doc.unlocked.contains(&s.rn);
                let here = last.as_deref() == Some(s.rn.as_str());
                let label = if here {
                    format!("★ {}", s.name)
                } else {
                    s.name.clone()
                };
                if ui.checkbox(&mut on, label).changed() {
                    if on {
                        doc.unlocked.insert(s.rn.clone());
                    } else {
                        doc.unlocked.remove(&s.rn);
                    }
                }
            }
        });
    ui.add_space(6.0);
    ui.weak("Tick stations to unlock them, then Save/Download.");
}

/// Raw tab: a read-only dump of every top-level protobuf field.
pub(super) fn raw_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    ui.horizontal(|ui| {
        theme::page(ui, 20.0, accent);
        ui.label(
            egui::RichText::new("Raw fields")
                .color(accent)
                .size(18.0)
                .strong(),
        );
    });
    ui.weak(
        "Named top-level save fields. Single scalar fields (varint/string) are editable inline — \
         changes apply immediately. Advanced: back up first.",
    );
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.add(egui::TextEdit::singleline(&mut doc.raw_filter).hint_text("field name or number…"));
        if ui.button("✕").clicked() {
            doc.raw_filter.clear();
        }
    });
    ui.add_space(2.0);

    let needle = doc.raw_filter.to_lowercase();
    let fields = doc.save.raw_fields().unwrap_or_default();
    egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
        egui::Grid::new("raw_grid").num_columns(3).spacing([16.0, 4.0]).striped(true).show(ui, |ui| {
            ui.label(egui::RichText::new("field").color(accent).strong());
            ui.label(egui::RichText::new("type").color(accent).strong());
            ui.label(egui::RichText::new("value").color(accent).strong());
            ui.end_row();
            for f in &fields {
                let label = if f.name.is_empty() { format!("field {}", f.number) } else { f.name.to_string() };
                if !needle.is_empty()
                    && !label.to_lowercase().contains(&needle)
                    && !f.number.to_string().contains(&needle)
                {
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.monospace(&label);
                    let help = if f.help.is_empty() {
                        format!("field #{} — not documented in our schema; purpose unknown, so editing may have unpredictable effects.", f.number)
                    } else {
                        format!("{}\n\n(field #{})", f.help, f.number)
                    };
                    ui.label(egui::RichText::new(" ?").color(accent).small()).on_hover_text(help);
                });
                ui.weak(f.kind);
                if let Some(v) = f.value {
                    let mut v2 = v;
                    if ui.add(egui::DragValue::new(&mut v2).speed(1.0)).changed() {
                        // Refresh the scratch values so a field another tab also
                        // owns (level, money, …) isn't overwritten on save.
                        if doc.save.set_raw_varint(f.number, v2).is_ok() {
                            resync_scratch(doc);
                        }
                    }
                } else if let Some(t) = &f.text {
                    let mut t2 = t.clone();
                    if ui.text_edit_singleline(&mut t2).changed()
                        && doc.save.set_raw_string(f.number, &t2).is_ok()
                    {
                        resync_scratch(doc);
                    }
                } else {
                    ui.monospace(&f.preview);
                }
                ui.end_row();
            }
        });
    });
}

/// About tab: what this is, what it edits, credits, and the IP/asset policy.
pub(super) fn about_tab(ui: &mut egui::Ui, accent: egui::Color32) {
    ui.horizontal(|ui| {
        theme::emblem(ui, 28.0, accent);
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("BL2 Save Editor")
                .color(accent)
                .size(20.0)
                .strong(),
        );
    });
    ui.add_space(6.0);
    ui.label(
        "A Linux-native, dependency-light Borderlands 2 save editor written in Rust. It runs as a \
         web app (this page) and as a native desktop app from the same code — no Wine, Mono, or \
         installed game files required.",
    );
    ui.add_space(8.0);

    let head = |ui: &mut egui::Ui, s: &str| {
        ui.label(egui::RichText::new(s).color(accent).strong());
    };
    head(ui, "What it edits");
    for s in [
        "Character — name, class, head/skin, level, XP, skill points",
        "Currency — money, eridium, seraph crystals, torgue tokens",
        "Items — per-item level, parts, shareable BL2(…) codes, backpack ↔ bank",
        "General — playthroughs, OP level, backpack/bank slots, save info",
        "Fast Travel — unlock stations (base game + DLC)",
        "Vehicle — equip vehicle skins (Runner / Technical / Hovercraft / Fan Boat)",
        "Profile (drag profile.bin) — Golden Keys, Badass Rank, unlock all customizations",
    ] {
        ui.label(format!("   •  {s}"));
    }
    ui.add_space(8.0);
    head(ui, "Credits");
    ui.label(
        "The Borderlands 2 save/profile formats and item/game data were figured out by the \
         community. This tool reimplements them in Rust — a native Linux app that also builds for \
         Windows and the web. Huge thanks to:",
    );
    for c in [
        "Gibbed (rick) — Gibbed.Borderlands2 save editor & GameInfo data (zlib licence)",
        "apocalyptech — Python BL2 editor (cross-checked the save format & fields)",
        "withmorten (B2Profile) — the profile.bin format",
        "Community code lists — the built-in item-code library",
    ] {
        ui.label(format!("   •  {c}"));
    }
    ui.add_space(8.0);
    head(ui, "Art & data");
    ui.label(
        "Every icon and theme is original art drawn in code — no Gearbox/2K assets are bundled. \
         Item/part names are open identifier data. No third-party code is copied; formats and \
         field IDs are facts, reimplemented cleanly. Borderlands 2 © Gearbox/2K; unaffiliated.",
    );
    ui.add_space(10.0);
    ui.colored_label(theme::DANGER, "⚠ Always back up your save before editing.");
}

/// Rebuild the item views from the save after a structural change (e.g. import),
/// preserving any unsaved per-item level scratch edits by id.
pub(super) fn rebuild_items_preserving_levels(doc: &mut Doc) {
    let old: std::collections::HashMap<usize, i64> =
        doc.items.iter().map(|v| (v.id, v.level)).collect();
    doc.items = build_item_views(&doc.save);
    for v in &mut doc.items {
        if let Some(&lv) = old.get(&v.id) {
            v.level = lv;
        }
    }
}

/// The item detail form: a read-only header (type/balance/manufacturer/level)
/// plus this item's part slots, each named via [`bl2_save::slot_label`] with a
/// "change" button. The button only opens the picker modal — the swap itself
/// happens there.
pub(super) fn parts_editor(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    let Some(id) = doc.editing_parts else { return };
    let Some(idx) = doc.items.iter().position(|v| v.id == id) else {
        doc.editing_parts = None;
        return;
    };
    let v = &doc.items[idx];
    let item_name = v.display.clone();
    let is_weapon = v.is_weapon;
    let (kind, type_name, balance, manufacturer, level) = (
        if is_weapon { "Weapon" } else { "Item" },
        v.type_name.clone(),
        v.balance.clone(),
        v.manufacturer.clone(),
        v.level,
    );
    let slots: Vec<(usize, String)> = v.parts.iter().map(|p| (p.slot, p.name.clone())).collect();

    ui.add_space(6.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("Details — {item_name}"))
                .color(accent)
                .strong(),
        );
        if ui.button("Close").clicked() {
            doc.editing_parts = None;
            doc.editing_part_slot = None;
        }
    });
    if doc.editing_parts.is_none() {
        return;
    }

    // Read-only header: what this item is.
    egui::Grid::new("item_header_grid")
        .num_columns(2)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            let field = |ui: &mut egui::Ui, k: &str, val: &str| {
                ui.label(egui::RichText::new(k).color(accent));
                ui.monospace(if val.is_empty() { "—" } else { val });
                ui.end_row();
            };
            field(ui, "Kind", kind);
            field(ui, "Type", &type_name);
            field(ui, "Balance", &balance);
            if !manufacturer.is_empty() {
                field(ui, "Manufacturer", &manufacturer);
            }
            field(ui, "Level", &format!("{level}"));
        });

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Parts").color(accent).strong());
    ui.colored_label(
        theme::DANGER,
        "⚠ Changing parts can create items the game rejects — back up first and verify in-game.",
    );
    egui::Grid::new("parts_grid")
        .num_columns(3)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            for (slot, name) in &slots {
                ui.label(egui::RichText::new(bl2_save::slot_label(is_weapon, *slot)).strong());
                ui.monospace(name);
                if ui.small_button("change").clicked() {
                    doc.editing_part_slot = Some(*slot);
                    doc.part_filter.clear();
                }
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCENT: egui::Color32 = egui::Color32::from_rgb(245, 177, 30);
    const TEXT: egui::Color32 = egui::Color32::from_rgb(230, 226, 214);

    /// Run a tab against a real egui `Ui`, with no window and no GPU.
    ///
    /// The widget code executes in full — grids, combos, scroll areas, drag
    /// values — so anything that would panic on real save data (bad indexing,
    /// a stray unwrap, broken geometry) fails the calling test. What the frame
    /// *looks* like isn't asserted; the checks below use the state each tab
    /// leaves behind, which is the part that reaches the save file.
    fn headless<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        let mut f = Some(f);
        let mut out = None;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            if let Some(f) = f.take() {
                out = Some(f(ui));
            }
        });
        out.expect("the ui closure ran")
    }

    /// A Gaige carrying real items in both bags, so the item paths have data.
    fn doc_with_items() -> Doc {
        let mut save = SaveFile::new_character(
            "GD_Tulip_Mechromancer.Character.CharClass_Mechromancer",
            "Gaige",
        );
        for e in bl2_save::code_library().iter().take(4) {
            save.add_item_from_code(&e.code, false).unwrap();
        }
        save.add_item_from_code(&bl2_save::code_library()[4].code, true)
            .unwrap();
        doc_from_save(save, "save0001.sav".to_string(), None)
    }

    #[test]
    fn every_tab_renders_over_real_save_data() {
        let mut doc = doc_with_items();
        let items_before = doc.items.len();
        headless(|ui| character_tab(&mut doc, ui, ACCENT));
        headless(|ui| general_tab(&mut doc, ui, ACCENT));
        headless(|ui| currency_tab(&mut doc, ui, ACCENT));
        headless(|ui| vehicle_tab(&mut doc, ui, ACCENT));
        headless(|ui| fast_travel_tab(&mut doc, ui, ACCENT));
        headless(|ui| raw_tab(&mut doc, ui, ACCENT));
        headless(|ui| about_tab(ui, ACCENT));
        let status = headless(|ui| items_tab(&mut doc, ui, ACCENT, TEXT));

        assert!(status.is_none(), "no status without interaction");
        assert_eq!(doc.items.len(), items_before, "no items gained or lost");
        assert_eq!(doc.char_name, "Gaige", "the name survived a render pass");
        assert!(doc.save.to_bytes().is_ok(), "the save is still encodable");
    }

    #[test]
    fn tabs_render_on_a_character_with_no_items() {
        // The empty-backpack branches have to hold up too.
        let mut doc = doc_from_save(
            SaveFile::new_character("GD_Soldier.Character.CharClass_Soldier", "Axton"),
            "save0002.sav".to_string(),
            None,
        );
        assert!(doc.items.is_empty(), "a fresh character has no items");
        let status = headless(|ui| items_tab(&mut doc, ui, ACCENT, TEXT));
        assert!(status.is_none());
        headless(|ui| character_tab(&mut doc, ui, ACCENT));
        headless(|ui| raw_tab(&mut doc, ui, ACCENT));
        headless(|ui| vehicle_tab(&mut doc, ui, ACCENT));
        headless(|ui| fast_travel_tab(&mut doc, ui, ACCENT));
        assert!(doc.items.is_empty(), "still no items");
    }

    #[test]
    fn rendering_clamps_out_of_range_scratch_values() {
        // The tabs clamp as they draw, so a nonsense value can't reach the save.
        let mut doc = doc_with_items();
        doc.op_level = 99;
        doc.playthroughs_completed = 9;
        doc.backpack_size = 9_999;
        doc.bank_size = 9_999;
        headless(|ui| general_tab(&mut doc, ui, ACCENT));
        assert_eq!(doc.op_level, MAX_OP_LEVEL, "OP clamped to the game ceiling");
        assert_eq!(doc.playthroughs_completed, 3, "playthroughs clamped to 3");
        assert_eq!(doc.backpack_size, 39, "backpack clamped to the SDU max");
        assert_eq!(doc.bank_size, 200, "bank clamped");

        // Item level rows clamp into the 7-bit packed field's range.
        doc.items[0].level = 9_999;
        doc.item_level = 9_999;
        headless(|ui| items_tab(&mut doc, ui, ACCENT, TEXT));
        assert!(doc.items[0].level <= 127, "item level clamped");
        assert!(doc.item_level <= 127, "the bulk level target clamped");

        // Currency is capped at int32, which is what the save field holds.
        doc.money = i64::MAX;
        doc.eridium = i64::MAX;
        headless(|ui| currency_tab(&mut doc, ui, ACCENT));
        assert_eq!(doc.money, MAX, "money clamped to int32");
        assert_eq!(doc.eridium, MAX, "eridium clamped to int32");
    }

    #[test]
    fn rendering_leaves_valid_values_alone() {
        // Drawing must not quietly change anything already in range.
        let mut doc = doc_with_items();
        doc.level = 50;
        doc.xp = 3_429_728;
        doc.money = 99_999_999;
        doc.eridium = 500;
        doc.seraph = 999;
        doc.torgue = 999;
        doc.skill_points = 46;
        doc.op_level = 0;
        doc.playthroughs_completed = 2;
        doc.active_playthrough = 2;
        doc.backpack_size = 24;
        doc.bank_size = 24;
        // An array, not a tuple: tuples only compare up to 12 elements.
        let snapshot = |d: &Doc| -> [i64; 13] {
            [
                d.level,
                d.xp,
                d.money,
                d.eridium,
                d.seraph,
                d.torgue,
                d.skill_points,
                d.op_level,
                d.playthroughs_completed,
                d.active_playthrough,
                d.backpack_size,
                d.bank_size,
                d.unlocked.len() as i64,
            ]
        };
        let before = snapshot(&doc);
        headless(|ui| character_tab(&mut doc, ui, ACCENT));
        headless(|ui| general_tab(&mut doc, ui, ACCENT));
        headless(|ui| currency_tab(&mut doc, ui, ACCENT));
        headless(|ui| fast_travel_tab(&mut doc, ui, ACCENT));
        headless(|ui| vehicle_tab(&mut doc, ui, ACCENT));
        assert_eq!(
            before,
            snapshot(&doc),
            "a render pass changed an in-range value"
        );
    }

    #[test]
    fn combos_handle_known_unknown_and_empty_selections() {
        let class = "GD_Siren.Character.CharClass_Siren";
        let known = bl2_save::customizations(class, true)
            .first()
            .map(|c| c.path.clone())
            .expect("the siren has heads");
        // "0" is the save's placeholder, and an unresolvable path has to fall
        // back to its tail rather than panic.
        for value in [known.as_str(), "0", "", "GD_Nonsense.Some.Unknown_Path"] {
            let mut v = value.to_string();
            headless(|ui| customization_combo(ui, "h", class, true, &mut v));
            assert_eq!(v, value, "rendering must not rewrite the selection");
        }
        for value in ["None", "", "GD_Nonsense.Vehicle.Skin"] {
            let mut v = value.to_string();
            headless(|ui| vehicle_skin_combo(ui, "v", "Runner", &mut v));
            assert_eq!(v, value, "rendering must not rewrite the skin");
        }
        // An unknown class offers no options; the combo still has to render.
        let mut v = "0".to_string();
        headless(|ui| customization_combo(ui, "h", "nonsense", true, &mut v));
        // An unknown vehicle family likewise has an empty skin list.
        let mut v2 = "None".to_string();
        headless(|ui| vehicle_skin_combo(ui, "v", "NoSuchFamily", &mut v2));
    }

    #[test]
    fn parts_editor_opens_closes_and_survives_a_stale_id() {
        let mut doc = doc_with_items();
        doc.editing_parts = None;
        headless(|ui| parts_editor(&mut doc, ui, ACCENT));
        assert!(doc.editing_parts.is_none(), "stays closed");

        // Open on a real item.
        let id = doc.items[0].id;
        doc.editing_parts = Some(id);
        headless(|ui| parts_editor(&mut doc, ui, ACCENT));
        assert_eq!(doc.editing_parts, Some(id), "stays open on a live id");

        // An id that no longer exists closes the editor instead of panicking —
        // this is the path a stale selection takes after an item is removed.
        doc.editing_parts = Some(9_999);
        headless(|ui| parts_editor(&mut doc, ui, ACCENT));
        assert!(doc.editing_parts.is_none(), "a stale id closes the editor");
    }

    #[test]
    fn rebuild_items_preserves_scratch_levels_by_id() {
        let mut doc = doc_with_items();
        for (i, v) in doc.items.iter_mut().enumerate() {
            v.level = 30 + i as i64;
        }
        let want: Vec<(usize, i64)> = doc.items.iter().map(|v| (v.id, v.level)).collect();

        // A structural change (importing another item) rebuilds the views.
        doc.save
            .add_item_from_code(&bl2_save::code_library()[7].code, false)
            .unwrap();
        rebuild_items_preserving_levels(&mut doc);

        assert_eq!(doc.items.len(), want.len() + 1, "the new item shows up");
        for (id, level) in want {
            let got = doc.items.iter().find(|v| v.id == id).expect("item kept");
            assert_eq!(got.level, level, "pending level for id {id} survived");
        }
    }

    #[test]
    fn raw_tab_renders_for_matching_empty_and_missing_filters() {
        let mut doc = doc_with_items();
        for filter in ["", "level", "20", "zzzz-no-such-field", "CLASS"] {
            doc.raw_filter = filter.to_string();
            headless(|ui| raw_tab(&mut doc, ui, ACCENT));
            assert_eq!(doc.raw_filter, filter, "the filter text is not rewritten");
        }
        // The inspector must not have disturbed the save while listing it.
        assert_eq!(doc.save.level(), Some(1));
        assert!(doc.save.to_bytes().is_ok());
    }

    #[test]
    fn items_tab_renders_both_bags() {
        let mut doc = doc_with_items();
        assert!(
            doc.items.iter().any(|v| v.is_bank) && doc.items.iter().any(|v| !v.is_bank),
            "the fixture fills both bags"
        );
        for show_bank in [false, true] {
            doc.show_bank = show_bank;
            let status = headless(|ui| items_tab(&mut doc, ui, ACCENT, TEXT));
            assert!(status.is_none(), "no status without interaction");
            assert_eq!(doc.show_bank, show_bank, "rendering must not flip the bag");
        }
    }

    /// The profile view needs a decoded `profile.bin`, which can't be synthesised
    /// from outside `bl2-save` — so this runs only when a sample is present, the
    /// same way the core's golden tests do.
    #[test]
    fn profile_view_renders_a_real_profile_if_present() {
        let Some(raw) = ["../samples/profile.bin", "samples/profile.bin"]
            .iter()
            .find_map(|p| std::fs::read(p).ok())
        else {
            eprintln!("no sample profile.bin present, skipping");
            return;
        };
        let profile = ProfileFile::from_bytes(&raw).expect("the sample profile decodes");
        let mut pdoc = ProfileDoc {
            golden_keys: profile.golden_keys().unwrap_or(0) as i64,
            badass_rank: profile.badass_rank().unwrap_or(0) as i64,
            badass_tokens: profile.badass_tokens().unwrap_or(0) as i64,
            pending_customizations: None,
            name: "profile.bin".to_string(),
            path: None,
            profile,
        };
        headless(|ui| profile_view(&mut pdoc, ui, ACCENT));
        assert!(
            pdoc.pending_customizations.is_none(),
            "no pending change without a click"
        );

        // Golden keys clamp to a byte as they render.
        pdoc.golden_keys = 9_999;
        headless(|ui| profile_view(&mut pdoc, ui, ACCENT));
        assert_eq!(pdoc.golden_keys, 255, "golden keys clamped to 255");
    }
}
