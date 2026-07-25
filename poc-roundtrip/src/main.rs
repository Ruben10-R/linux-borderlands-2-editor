// Borderlands 2 save tool — proof-of-concept CLI.
//
// Superseded by the `bl2-save` crate and the `bl2edit` CLI; kept only as the
// original standalone proof that the codec round-trips. Don't build on it.
//
// Subcommands:
//   roundtrip [sav]                     decode + re-encode, prove it's byte-correct
//   dump      [sav]                     list the protobuf's top-level fields
//   info      [sav]                     show money + eridium
//   set-currency <in> <out> <money> <eridium>
//                                       edit currency, re-encode, write <out> (+ verify)
//
// Defaults the save path to ../samples/save0001.sav when omitted.

mod proto;
mod savefile;

use std::error::Error;
use std::fs;

use minilzo_rs::LZO;

const DEFAULT_SAV: &str = "../samples/save0001.sav";

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("roundtrip");

    match cmd {
        "roundtrip" => cmd_roundtrip(args.get(1).map(String::as_str).unwrap_or(DEFAULT_SAV)),
        "dump" => cmd_dump(args.get(1).map(String::as_str).unwrap_or(DEFAULT_SAV)),
        "info" => cmd_info(args.get(1).map(String::as_str).unwrap_or(DEFAULT_SAV)),
        "set-currency" => {
            if args.len() != 5 {
                return Err("usage: set-currency <in.sav> <out.sav> <money> <eridium>".into());
            }
            cmd_set_currency(&args[1], &args[2], args[3].parse()?, args[4].parse()?)
        }
        // If the first arg looks like a path, treat it as roundtrip <path>.
        other if other.ends_with(".sav") => cmd_roundtrip(other),
        other => {
            Err(format!("unknown command '{other}' (roundtrip|dump|info|set-currency)").into())
        }
    }
}

fn load_proto(path: &str, lzo: &LZO) -> Result<savefile::Decoded, Box<dyn Error>> {
    let raw = fs::read(path)?;
    let d = savefile::decode(&raw, lzo)?;
    if !d.is_valid() {
        return Err(format!(
            "{path}: decode failed validation (sha_ok={}, crc match={})",
            d.sha_ok,
            d.crc_stored == d.crc_calc
        )
        .into());
    }
    Ok(d)
}

fn cmd_roundtrip(path: &str) -> Result<(), Box<dyn Error>> {
    let mut lzo = LZO::init()?;
    let raw = fs::read(path)?;
    println!("== INPUT: {} ({} bytes) ==", path, raw.len());

    let d = savefile::decode(&raw, &lzo)?;
    println!("\n-- Proof A: decode correctness --");
    println!("  WSG version              : {}", d.version);
    println!("  outer SHA1 matches file  : {}", yn(d.sha_ok));
    println!("  protobuf bytes decoded   : {}", d.proto.len());
    println!(
        "  CRC32 stored / computed  : {:#010x} / {:#010x}",
        d.crc_stored, d.crc_calc
    );
    let a_ok = d.is_valid();
    println!("  => decode matches game   : {}", yn(a_ok));

    let reencoded = savefile::encode(&d.proto, &mut lzo)?;
    let d2 = savefile::decode(&reencoded, &lzo)?;
    println!("\n-- Proof B: re-encode is a valid save --");
    println!("  re-encoded file size     : {} bytes", reencoded.len());
    let proto_identical = d2.proto == d.proto;
    println!("  protobuf byte-identical  : {}", yn(proto_identical));
    println!("  re-decoded SHA1 + CRC ok : {}", yn(d2.is_valid()));
    let b_ok = proto_identical && d2.is_valid();
    println!("  => re-encode is loadable : {}", yn(b_ok));

    println!("\n=====================================");
    if a_ok && b_ok {
        println!("ROUND-TRIP PROVEN");
        Ok(())
    } else {
        Err("round-trip assertions failed".into())
    }
}

// Re-read a varint at `start`, returning (value, bytes_consumed).
fn peek_varint(buf: &[u8], start: usize) -> (u64, usize) {
    let mut p = start;
    let mut v: u64 = 0;
    let mut sh = 0u32;
    loop {
        let b = buf[p];
        p += 1;
        v |= ((b & 0x7f) as u64) << sh;
        if b & 0x80 == 0 {
            break;
        }
        sh += 7;
    }
    (v, p - start)
}

fn cmd_dump(path: &str) -> Result<(), Box<dyn Error>> {
    let lzo = LZO::init()?;
    let d = load_proto(path, &lzo)?;
    let fields = proto::parse_fields(&d.proto)?;
    println!(
        "== {} : {} top-level protobuf fields ==",
        path,
        fields.len()
    );
    println!(
        "  {:>5}  {:>4}  {:>8}  value/notes",
        "field", "wire", "size"
    );
    for f in &fields {
        let size = f.end - f.val_start;
        let note = match f.wire_type {
            0 => {
                let (v, _) = peek_varint(&d.proto, f.val_start);
                format!("= {v}")
            }
            2 => {
                let (len, adv) = peek_varint(&d.proto, f.val_start);
                let content = &d.proto[f.val_start + adv..f.val_start + adv + len as usize];
                let printable = content.iter().all(|&c| c == 0 || (0x20..0x7f).contains(&c));
                if printable && !content.is_empty() {
                    format!(
                        "\"{}\"",
                        String::from_utf8_lossy(content).replace('\0', "\\0")
                    )
                } else {
                    format!("({} bytes)", content.len())
                }
            }
            _ => String::new(),
        };
        println!(
            "  {:>5}  {:>4}  {:>8}  {}",
            f.number, f.wire_type, size, note
        );
    }
    Ok(())
}

fn cmd_info(path: &str) -> Result<(), Box<dyn Error>> {
    let lzo = LZO::init()?;
    let d = load_proto(path, &lzo)?;
    let fields = proto::parse_fields(&d.proto)?;
    let currency = proto::read_currency(&d.proto, &fields)?;
    println!("== {} ==", path);
    if let Some(class_def) = proto::read_string_field(&d.proto, &fields, 1) {
        println!("  class   : {}", proto::class_name(&class_def));
    }
    if let Some(level) = proto::read_varint_field(&d.proto, &fields, 2) {
        println!("  level   : {level}");
    }
    if let Some(xp) = proto::read_varint_field(&d.proto, &fields, 3) {
        println!("  xp      : {xp}");
    }
    let money = currency.get(proto::IDX_MONEY).copied().unwrap_or(0);
    let eridium = currency.get(proto::IDX_ERIDIUM).copied().unwrap_or(0);
    println!("  money   : {money}");
    println!("  eridium : {eridium}");
    Ok(())
}

fn cmd_set_currency(
    input: &str,
    output: &str,
    money: i64,
    eridium: i64,
) -> Result<(), Box<dyn Error>> {
    let mut lzo = LZO::init()?;
    let d = load_proto(input, &lzo)?;
    let fields = proto::parse_fields(&d.proto)?;

    let mut currency = proto::read_currency(&d.proto, &fields)?;
    while currency.len() <= proto::IDX_ERIDIUM {
        currency.push(0);
    }
    let old = currency.clone();
    currency[proto::IDX_MONEY] = money;
    currency[proto::IDX_ERIDIUM] = eridium;

    let new_proto = proto::rewrite_currency(&d.proto, &fields, &currency)?;

    // Safety: everything except the currency field must be untouched.
    verify_only_currency_changed(&d.proto, &new_proto)?;

    // Re-encode and prove the new file decodes cleanly with the new values.
    let file = savefile::encode(&new_proto, &mut lzo)?;
    let check = savefile::decode(&file, &lzo)?;
    if !check.is_valid() || check.proto != new_proto {
        return Err("re-encoded file failed self-check".into());
    }
    let check_currency = proto::read_currency(&check.proto, &proto::parse_fields(&check.proto)?)?;

    fs::write(output, &file)?;

    println!("== set-currency ==");
    println!("  input   : {input}");
    println!(
        "  money   : {} -> {}",
        old.first().copied().unwrap_or(0),
        money
    );
    println!(
        "  eridium : {} -> {}",
        old.get(1).copied().unwrap_or(0),
        eridium
    );
    println!(
        "  verify  : non-currency bytes unchanged, re-decode valid, currency now {check_currency:?}"
    );
    println!("  wrote   : {output} ({} bytes)", file.len());
    println!("\nNext: back up your live save, copy {output} into the savedata dir, launch BL2.");
    Ok(())
}

/// Confirm the only difference between two protobufs is the currency field.
fn verify_only_currency_changed(old: &[u8], new: &[u8]) -> Result<(), Box<dyn Error>> {
    let of = proto::parse_fields(old)?;
    let nf = proto::parse_fields(new)?;
    let others_old: Vec<&[u8]> = of
        .iter()
        .filter(|f| f.number != proto::CURRENCY_FIELD)
        .map(|f| &old[f.tag_start..f.end])
        .collect();
    let others_new: Vec<&[u8]> = nf
        .iter()
        .filter(|f| f.number != proto::CURRENCY_FIELD)
        .map(|f| &new[f.tag_start..f.end])
        .collect();
    if others_old != others_new {
        return Err("edit altered non-currency fields — refusing to write".into());
    }
    Ok(())
}

fn yn(b: bool) -> &'static str {
    if b {
        "YES"
    } else {
        "NO"
    }
}
