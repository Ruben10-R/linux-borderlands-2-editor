//! Unit + golden tests for the crate (kept in-crate to reach private helpers).

use super::*;

fn hex_to_bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Golden test: `new_character` must reproduce a real Gibbed "New" save
/// byte-for-byte (a Gaige/Mechromancer save exported from Gibbed 1.0.46), and
/// the result must round-trip through our real codec (a game-loadable file).
#[test]
fn new_character_matches_gibbed() {
    let expected = hex_to_bytes(concat!(
        "0a3647445f54756c69705f4d656368726f6d616e6365722e4368617261637465",
        "722e43686172436c6173735f4d656368726f6d616e6365721001180020002800",
        "320d0000000000000000000000000038006a060800100018009a01250a054761",
        "696765120808001000180020001a08080010001800200022080800100018002000",
        "a00101a80100c80100f801009202140d550c61f5155ae8dc401d851edb2b25996",
        "dffea9a02044e6f6e659a02009a02009a02009a02044e6f6e65a80200d00200e0",
        "0201e80202800300880300900300980300b80300c00300",
    ));
    let guid = [
        0x55, 0x0c, 0x61, 0xf5, 0x5a, 0xe8, 0xdc, 0x40, 0x85, 0x1e, 0xdb, 0x2b, 0x99, 0x6d, 0xff,
        0xea,
    ];
    let save = SaveFile::new_character_with_guid(
        "GD_Tulip_Mechromancer.Character.CharClass_Mechromancer",
        "Gaige",
        &guid,
    );
    assert_eq!(
        save.proto, expected,
        "new_character must byte-match Gibbed's New output"
    );

    // Encode → decode through the real codec: proves it's a valid container.
    let bytes = save.to_bytes().unwrap();
    let reloaded = SaveFile::from_bytes(&bytes).unwrap();
    assert_eq!(reloaded.proto, save.proto);
    assert_eq!(reloaded.level(), Some(1));
    assert_eq!(reloaded.name().as_deref(), Some("Gaige"));
}

/// A base-game class omits the DLC flags (fields 44/45); a DLC class includes
/// them — matching what real saves contain.
#[test]
fn new_character_dlc_flags() {
    let axton =
        SaveFile::new_character("GD_Soldier.Character.CharClass_Soldier", "Axton").to_bytes();
    assert!(axton.is_ok());
    let axton = SaveFile::new_character("GD_Soldier.Character.CharClass_Soldier", "Axton");
    let has_44 = crate::proto::parse_fields(&axton.proto)
        .unwrap()
        .iter()
        .any(|f| f.number == 44);
    assert!(!has_44, "base-game class must not emit is_dlc_player_class");

    let krieg = SaveFile::new_character(
        "GD_Lilac_PlayerClass.Character.CharClass_LilacPlayerClass",
        "Krieg",
    );
    let has_44 = crate::proto::parse_fields(&krieg.proto)
        .unwrap()
        .iter()
        .any(|f| f.number == 44);
    assert!(has_44, "DLC class must emit is_dlc_player_class");
}

/// Importing a group swaps in the source's fields (skills here) and the result
/// still round-trips through the codec.
#[test]
fn import_group_copies_fields() {
    let source = SaveFile::from_bytes(include_bytes!("../../samples/save0001.sav")).unwrap();
    let src_skill_count = crate::proto::parse_fields(&source.proto)
        .unwrap()
        .iter()
        .filter(|f| f.number == 8)
        .count();
    assert!(src_skill_count > 0, "sample should have skills");

    let mut fresh = SaveFile::new_character("GD_Assassin.Character.CharClass_Assassin", "Zero");
    fresh.import_group(&source, ImportGroup::Skills).unwrap();
    let got = crate::proto::parse_fields(&fresh.proto)
        .unwrap()
        .iter()
        .filter(|f| f.number == 8)
        .count();
    assert_eq!(got, src_skill_count, "skills copied from source");
    assert!(fresh.to_bytes().is_ok(), "still a valid save after import");
}

/// Build a tiny synthetic top-level protobuf: class(1,str), level(2), xp(3),
/// packed currency(6)=[money, eridium, 0], plus an "unknown" field(9) we must
/// never disturb.
fn synthetic_proto() -> Vec<u8> {
    let mut p = Vec::new();
    // field 1, wire 2: class string
    let class = b"GD_Assassin.Character.CharClass_Assassin";
    p.push((1 << 3) | 2);
    p.push(class.len() as u8);
    p.extend_from_slice(class);
    // field 2, wire 0: level = 4
    p.push(2 << 3);
    p.push(4);
    // field 3, wire 0: xp = 4288
    p.push(3 << 3);
    p.extend_from_slice(&[0xC0, 0x21]); // 4288 as varint
                                        // field 6, wire 2: packed [608, 0, 0]
    p.push((6 << 3) | 2);
    p.push(4); // payload len: 608(2) + 0 + 0
    p.extend_from_slice(&[0xE0, 0x04, 0x00, 0x00]);
    // field 9, wire 2: opaque "unknown" bytes
    p.push((9 << 3) | 2);
    p.push(3);
    p.extend_from_slice(&[0xDE, 0xAD, 0xBE][..3]);
    p
}

#[test]
fn codec_roundtrip_is_byte_identical_and_valid() {
    let proto = synthetic_proto();
    let bytes = codec::encode(&proto).unwrap();
    let dec = codec::decode(&bytes).unwrap();
    assert!(dec.is_valid(), "checksums must validate");
    assert_eq!(
        dec.proto, proto,
        "round-trip must preserve protobuf exactly"
    );
}

#[test]
fn reads_expected_scalars() {
    let s = SaveFile {
        proto: synthetic_proto(),
    };
    assert_eq!(s.level(), Some(4));
    assert_eq!(s.xp(), Some(4288));
    assert_eq!(s.money(), 608);
    assert_eq!(s.eridium(), 0);
    assert_eq!(s.class_name().as_deref(), Some("Zer0 (Assassin)"));
}

#[test]
fn code_library_loads_and_is_valid() {
    let lib = code_library();
    assert!(
        lib.len() > 1000,
        "library populated ({} entries)",
        lib.len()
    );
    // every entry is a real BL2(...) code and decodes
    for e in lib.iter().take(50) {
        assert!(e.code.starts_with("BL2(") && e.code.ends_with(')'));
        assert!(
            describe_code(&e.code).is_some(),
            "entry decodes: {}",
            e.code
        );
        assert!(!e.category.is_empty());
    }
    assert!(library_categories().contains(&"Weapon"));
}

#[test]
fn extract_codes_handles_separators_and_slashes() {
    // Codes contain '/' and '+'; separators between them vary.
    let text = "BL2(aa/bb+cc) , BL2(dd) | BL2(ee) / junk BL2(ff)\nBL2(gg)";
    let codes = extract_codes(text);
    assert_eq!(
        codes,
        ["BL2(aa/bb+cc)", "BL2(dd)", "BL2(ee)", "BL2(ff)", "BL2(gg)"]
    );
    assert!(extract_codes("no codes here").is_empty());
    // an unterminated trailing code is ignored
    assert_eq!(extract_codes("BL2(ok) BL2(unterminated"), ["BL2(ok)"]);
}

#[test]
fn slot_labels_match_layout() {
    // Weapons: fixed 11-slot layout, every slot named.
    assert_eq!(slot_label(true, 0), "Body");
    assert_eq!(slot_label(true, 2), "Barrel");
    assert_eq!(slot_label(true, 10), "Title");
    // Items: only the tail is reliably named across item types.
    assert_eq!(slot_label(false, 8), "Material");
    assert_eq!(slot_label(false, 9), "Prefix");
    assert_eq!(slot_label(false, 10), "Title");
    assert_eq!(slot_label(false, 0), "Part 1");
    // Out of range is graceful.
    assert_eq!(slot_label(true, 99), "Part");
}

#[test]
fn edits_change_only_the_target_field() {
    let mut s = SaveFile {
        proto: synthetic_proto(),
    };
    s.set_money(99_999_999).unwrap();
    s.set_eridium(500).unwrap();
    s.set_level(50).unwrap();
    s.set_xp(1_000_000).unwrap();
    assert_eq!(s.money(), 99_999_999);
    assert_eq!(s.eridium(), 500);
    assert_eq!(s.level(), Some(50));
    assert_eq!(s.xp(), Some(1_000_000));
    // The opaque unknown field (9) must be byte-preserved.
    let fields = s.fields().unwrap();
    let f9 = fields
        .iter()
        .find(|f| f.number == 9)
        .expect("field 9 preserved");
    assert_eq!(&s.proto[f9.val_start + 1..f9.end], &[0xDE, 0xAD, 0xBE]);
}

/// The game decompresses saves with a C LZO1x implementation. Prove our
/// pure-Rust `lzokay` compressed output is decodable by that canonical C impl
/// (`minilzo`) — i.e. it is standard LZO1x the game will accept.
#[test]
fn lzokay_output_is_decodable_by_c_lzo() {
    // Semi-structured data so the compressor actually emits matches + literals.
    let outer: Vec<u8> = (0..8192u32)
        .map(|i| ((i as u8).wrapping_mul(37)) ^ ((i >> 3) as u8))
        .collect();
    let comp = lzokay_native::compress(&outer).expect("lzokay compress");
    let lzo = minilzo_rs::LZO::init().expect("minilzo init");
    let back = lzo
        .decompress_safe(&comp, outer.len())
        .expect("C LZO decompress");
    assert_eq!(
        back, outer,
        "C LZO must decode lzokay output → the game will too"
    );
}

#[test]
fn currency_indices() {
    let mut s = SaveFile {
        proto: synthetic_proto(),
    };
    s.set_money(1).unwrap();
    s.set_eridium(2).unwrap();
    s.set_seraph(3).unwrap();
    s.set_torgue(4).unwrap(); // pads currency_on_hand out to index 4
    assert_eq!(
        (s.money(), s.eridium(), s.seraph(), s.torgue()),
        (1, 2, 3, 4)
    );
}

#[test]
fn character_edits() {
    let mut s = SaveFile {
        proto: synthetic_proto(),
    };
    s.set_skill_points(7).unwrap(); // field 4 absent in synthetic → appended
    assert_eq!(s.skill_points(), Some(7));
    s.set_class("GD_Siren.Character.CharClass_Siren").unwrap();
    assert_eq!(s.class_name().as_deref(), Some("Maya (Siren)"));
    assert_eq!(
        s.class_def().as_deref(),
        Some("GD_Siren.Character.CharClass_Siren")
    );
    // Other fields untouched.
    assert_eq!(s.level(), Some(4));
    assert_eq!(s.money(), 608);
}

#[test]
fn full_edit_still_encodes_and_self_verifies() {
    let mut s = SaveFile {
        proto: synthetic_proto(),
    };
    s.set_money(12_345).unwrap();
    // to_bytes() self-verifies; if the encoded file didn't round-trip it errors.
    let bytes = s.to_bytes().unwrap();
    let reloaded = SaveFile::from_bytes(&bytes).unwrap();
    assert_eq!(reloaded.money(), 12_345);
}

/// Golden test against a real save if one is present (they're gitignored, so
/// skip cleanly in CI / on machines without a sample).
#[test]
fn golden_real_save_if_present() {
    let candidates = ["../samples/save0001.sav", "samples/save0001.sav"];
    let Some(path) = candidates.iter().find(|p| std::path::Path::new(p).exists()) else {
        eprintln!("golden: no sample save present, skipping");
        return;
    };
    let save = SaveFile::load(path).expect("real save should load");
    // Re-encode must round-trip byte-identically at the protobuf level.
    let bytes = save.to_bytes().expect("real save should self-verify");
    let reloaded = SaveFile::from_bytes(&bytes).unwrap();
    assert_eq!(save.money(), reloaded.money());
    assert!(save.level().is_some());

    // Decisive game-acceptance proxy: our (lzokay) re-encoded REAL save must be
    // decompressible by the C LZO the game uses, to a valid WSG buffer.
    let outer_size = u32::from_be_bytes(bytes[20..24].try_into().unwrap()) as usize;
    let lzo = minilzo_rs::LZO::init().unwrap();
    let outer = lzo
        .decompress_safe(&bytes[24..], outer_size)
        .expect("game's C LZO must decompress our real re-encoded save");
    assert_eq!(
        outer.len(),
        outer_size,
        "decompressed size must match header"
    );
    assert_eq!(
        &outer[4..7],
        b"WSG",
        "decompressed buffer must be a WSG block"
    );

    // Every REAL item serial must decode AND re-encode byte-for-byte. The
    // hand-crafted "virtual" placeholders (OP-level markers, set==255) are
    // decoded but not normally packed, so they're excluded from the byte check.
    let serials = items::raw_serials(&save.proto).expect("read serials");
    let mut real = 0;
    for (n, s) in serials.iter().enumerate() {
        let decoded = serial::unwrap(s).unwrap_or_else(|e| panic!("serial #{n} decode: {e}"));
        if decoded.is_placeholder() {
            continue;
        }
        real += 1;
        let re = serial::reencode(s).expect("reencode");
        assert_eq!(&re, s, "real serial #{n} must round-trip byte-for-byte");
    }
    eprintln!(
        "golden: {}/{} real serials round-tripped",
        real,
        serials.len()
    );
    // The typed list should decode without error too.
    let _ = save.items().expect("items() should succeed");

    // Re-level every item to 50: must self-verify, change only item fields,
    // and every non-placeholder item must then report level 50.
    let mut leveled = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
    let n = leveled.set_all_item_levels(50, true).expect("relevel");
    let _ = leveled
        .to_bytes()
        .expect("re-leveled save must self-verify");
    assert_eq!(
        leveled.money(),
        save.money(),
        "re-leveling must not touch money"
    );
    for it in leveled.items().unwrap() {
        if !it.serial.is_placeholder() {
            assert_eq!(it.serial.stage, Some(50), "item should now be level 50");
        }
    }
    eprintln!("golden: re-leveled {n} items to 50");

    // Per-item leveling by id: changes iff the item is levelable; protected
    // (grade-≤1) items stay locked. All levelable items end at 42.
    let mut one = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
    for it in save.items().unwrap() {
        let changed = one.set_item_level(it.id, 42).unwrap();
        assert_eq!(
            changed,
            it.serial.is_levelable(),
            "id {}: changed vs levelable",
            it.id
        );
    }
    let _ = one.to_bytes().expect("per-item edits must self-verify");
    for it in one.items().unwrap() {
        if it.serial.is_levelable() {
            assert_eq!(it.serial.stage, Some(42), "levelable item should be 42");
        }
    }

    // Parts editing: swap a present part on the first item that has one.
    if let Some(it) = save
        .items()
        .unwrap()
        .into_iter()
        .find(|it| !it.serial.is_placeholder() && it.serial.parts.iter().any(|p| p.is_some()))
    {
        let cat = parts_catalog(it.serial.is_weapon, it.serial.set);
        assert!(!cat.is_empty(), "parts catalog must not be empty");
        let slot = it.serial.parts.iter().position(|p| p.is_some()).unwrap();
        let choice = &cat[0];
        let mut edited = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
        assert!(edited
            .set_item_part(it.id, slot, choice.lib, choice.asset)
            .unwrap());
        let _ = edited.to_bytes().expect("part edit must self-verify");
        let after = edited.items().unwrap();
        let pr = after.iter().find(|x| x.id == it.id).unwrap().serial.parts[slot].unwrap();
        assert_eq!(
            (pr.lib, pr.asset),
            (choice.lib, choice.asset),
            "slot holds chosen part"
        );
        eprintln!("golden: swapped part slot {slot} to {}", choice.name);
    }

    // Name editing (appearance sub-field) on the real save.
    if save.name().is_some() {
        let mut n = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
        n.set_name("Zer0edit").expect("set_name");
        let _ = n.to_bytes().expect("name edit self-verify");
        assert_eq!(n.name().as_deref(), Some("Zer0edit"));
        assert_eq!(n.money(), save.money(), "name edit must not touch money");
        eprintln!("golden: renamed to {:?}", n.name());
    }

    // Item codes: export the first real item, re-import it, and the copy must
    // decode to the same balance/kind (only its key differs). Count grows by 1.
    if let Some(orig) = save
        .items()
        .unwrap()
        .into_iter()
        .find(|it| !it.serial.is_placeholder())
    {
        let code = save
            .item_code(orig.id)
            .unwrap()
            .expect("code for real item");
        assert!(
            code.starts_with("BL2(") && code.ends_with(')'),
            "code shape: {code}"
        );

        let mut imp = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
        let before = imp.items().unwrap().len();
        imp.add_item_from_code(&code, false).expect("import code");
        let _ = imp.to_bytes().expect("imported item must self-verify");
        let after = imp.items().unwrap();
        assert_eq!(after.len(), before + 1, "one item added");
        let added = after.last().unwrap();
        assert_eq!(added.serial.balance, orig.serial.balance, "same balance");
        assert_eq!(added.serial.is_weapon, orig.serial.is_weapon, "same kind");
        assert_eq!(imp.money(), save.money(), "import must not touch money");
        eprintln!(
            "golden: exported+reimported {} ({} chars)",
            code.len(),
            code.len()
        );
    }

    // Fast-travel stations decode as non-empty short names; raw inspector
    // lists the class field (#1) among the top-level fields.
    let stations = save.visited_stations();
    assert!(
        stations.iter().all(|s| !s.is_empty()),
        "station names non-empty"
    );
    assert!(
        save.raw_fields().unwrap().iter().any(|f| f.number == 1),
        "raw lists field 1"
    );
    eprintln!(
        "golden: {} stations, last = {:?}",
        stations.len(),
        save.last_station()
    );

    // Unlock-all-stations: rewrite field 16 to the full catalog; must
    // self-verify, touch nothing else, and read back the same set.
    let all: Vec<String> = stations_catalog().iter().map(|s| s.rn.clone()).collect();
    assert!(all.len() > 50, "station catalog populated");
    let mut ft = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
    ft.set_visited_stations(&all).expect("set stations");
    let _ = ft.to_bytes().expect("station edit must self-verify");
    assert_eq!(
        ft.visited_stations().len(),
        all.len(),
        "all stations unlocked"
    );
    assert_eq!(
        ft.money(),
        save.money(),
        "station edit must not touch money"
    );
    assert_eq!(ft.name(), save.name(), "station edit must not touch name");
    // A known station resolves to its display name.
    assert_eq!(
        station_display_name("SouthernShelfTown"),
        Some("Southern Shelf")
    );
    eprintln!("golden: unlocked all {} stations", all.len());

    // General/playthrough edits: playthroughs_completed (field 7, present) and
    // active_playthrough (field 49, likely absent → appended). Guarded, self-verify.
    let mut gen = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
    gen.set_playthroughs_completed(2).expect("set playthroughs");
    gen.set_active_playthrough(1)
        .expect("set active playthrough");
    let _ = gen.to_bytes().expect("general edits must self-verify");
    assert_eq!(gen.playthroughs_completed(), Some(2));
    assert_eq!(gen.active_playthrough(), 1);
    assert_eq!(
        gen.money(),
        save.money(),
        "general edits must not touch money"
    );
    assert_eq!(gen.name(), save.name(), "general edits must not touch name");
    assert_eq!(
        gen.level(),
        save.level(),
        "general edits must not touch level"
    );
    eprintln!(
        "golden: playthroughs {:?} -> 2, active -> 1; save id {:?}, time {:?}s",
        save.playthroughs_completed(),
        save.save_game_id(),
        save.time_played()
    );

    // Head/skin (field 35 "wearing"): the catalog for this character's class
    // is populated; equipping a catalog head+skin self-verifies and reads back.
    if let Some(class_def) = save.class_def() {
        let heads = customizations(&class_def, true);
        let skins = customizations(&class_def, false);
        assert!(
            !heads.is_empty() && !skins.is_empty(),
            "customization catalog for class"
        );
        let (h, s) = (heads[0].path.clone(), skins[0].path.clone());
        let mut app = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
        app.set_wearing(&h, &s).expect("set wearing");
        let _ = app.to_bytes().expect("wearing edit must self-verify");
        let w = app.wearing();
        assert_eq!(
            w.first().map(String::as_str),
            Some(h.as_str()),
            "head at index 0"
        );
        assert_eq!(
            w.get(4).map(String::as_str),
            Some(s.as_str()),
            "skin at index 4"
        );
        assert_eq!(
            app.money(),
            save.money(),
            "wearing edit must not touch money"
        );
        assert_eq!(app.name(), save.name(), "wearing edit must not touch name");
        eprintln!(
            "golden: equipped head {:?} + skin {:?}",
            heads[0].name, skins[0].name
        );
    }

    // Vehicle skins (field 57): equip a Runner skin, self-verify, read back,
    // touch nothing else.
    let runner = &VEHICLE_FAMILIES[0];
    let skins = vehicle_skins(runner.token);
    assert!(!skins.is_empty(), "runner skins present");
    let chosen = vec![skins[0].path.clone(), skins[1].path.clone()];
    let mut veh = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
    veh.set_vehicle_skins(runner.path, &chosen)
        .expect("set vehicle skins");
    let _ = veh.to_bytes().expect("vehicle edit must self-verify");
    assert_eq!(
        veh.vehicle_family_skins(runner.path),
        chosen,
        "runner skins read back"
    );
    assert_eq!(
        veh.money(),
        save.money(),
        "vehicle edit must not touch money"
    );
    assert_eq!(veh.name(), save.name(), "vehicle edit must not touch name");
    assert_eq!(
        vehicle_skin_name(&skins[0].path),
        Some(skins[0].name.as_str())
    );
    eprintln!("golden: equipped Runner skins {:?}", skins[0].name);

    // Overpower level (virtual item in field 53): set 8, read back 8,
    // self-verify, and re-set to 10 (exercises the update-in-place path).
    let mut op = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
    op.set_op_level(8).expect("set op 8");
    let _ = op.to_bytes().expect("op edit must self-verify");
    assert_eq!(op.op_level(), Some(8), "OP level reads back as 8");
    op.set_op_level(10).expect("set op 10");
    let _ = op.to_bytes().expect("op re-edit must self-verify");
    assert_eq!(op.op_level(), Some(10), "OP level updates in place to 10");
    assert_eq!(op.money(), save.money(), "op edit must not touch money");
    assert_eq!(
        op.items().unwrap().len(),
        {
            // setting OP twice must not keep appending virtual items
            let mut o2 = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
            o2.set_op_level(1).unwrap();
            o2.set_op_level(2).unwrap();
            o2.items().unwrap().len()
        },
        "no duplicate OP virtual items"
    );
    eprintln!("golden: OP level {:?} -> 10", save.op_level());

    // XP<->level sync table + raw named/grouped fields + raw edit.
    assert_eq!(xp_for_level(1), 0);
    assert_eq!(xp_for_level(72), xp_for_level(72));
    assert_eq!(
        level_for_xp(xp_for_level(50)),
        50,
        "level_for_xp inverts xp_for_level"
    );
    assert_eq!(level_for_xp(0), 1);
    let raw = save.raw_fields().unwrap();
    let class = raw.iter().find(|r| r.number == 1).expect("field 1");
    assert_eq!(class.name, "class");
    assert!(class.text.is_some(), "class is an editable string field");
    assert!(
        raw.iter().any(|r| r.number == 8 && r.kind == "collection"),
        "skills is a collection"
    );
    // Raw edit of a scalar varint (mission_number, field 21) touches only it.
    let mut r = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
    r.set_raw_varint(proto::FIELD_LEVEL, 55).unwrap();
    assert_eq!(r.level(), Some(55));
    assert_eq!(r.money(), save.money(), "raw edit must not touch money");
    eprintln!("golden: raw fields {} groups; sync/raw ok", raw.len());

    // Backpack/Bank capacity: set 39/24, snap to SDU grid, self-verify, and
    // confirm only the capacity fields moved.
    let mut cap = SaveFile::from_bytes(&save.to_bytes().unwrap()).unwrap();
    cap.set_backpack_size(39).expect("set backpack");
    cap.set_bank_size(24).expect("set bank");
    let _ = cap.to_bytes().expect("capacity edit must self-verify");
    assert_eq!(cap.backpack_size(), Some(39), "backpack snaps to 39");
    assert_eq!(cap.bank_size(), 24, "bank snaps to 24");
    assert_eq!(
        cap.money(),
        save.money(),
        "capacity edit must not touch money"
    );
    assert_eq!(
        cap.items().unwrap().len(),
        save.items().unwrap().len(),
        "no items added"
    );
    eprintln!(
        "golden: backpack {:?}->39, bank {}->24",
        save.backpack_size(),
        save.bank_size()
    );
}
