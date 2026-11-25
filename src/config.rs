//! `Config`: the Java mod's options, in `config/ironfurnaces.toml` with its keys and defaults. Read once; a missing
//! file is written with the defaults. The server's own file decides what its furnaces do.

use mod_api::*;
use std::collections::HashMap;
use std::sync::OnceLock;

const FILE: &str = "ironfurnaces.toml";

pub(crate) const DEFAULT: &str = "# Iron Furnaces' options, as the Java mod's.
misc.lightupdates = false
energy.tier_0 = 80000
energy.tier_1 = 200000
energy.tier_2 = 1000000
iron_furnace.speed = 160
gold_furnace.speed = 120
diamond_furnace.speed = 80
emerald_furnace.speed = 40
obsidian_furnace.speed = 20
crystal_furnace.speed = 40
netherite_furnace.speed = 5
copper_furnace.speed = 180
silver_furnace.speed = 140
rainbow_furnace.speed = 20
allthemodium_furnace.speed = 5
vibranium_furnace.speed = 3
unobtainium_furnace.speed = 1
allthemodium_furnace.mult = 16
vibranium_furnace.mult = 32
unobtainium_furnace.mult = 64
iron_furnace.generation = 40
gold_furnace.generation = 160
diamond_furnace.generation = 240
emerald_furnace.generation = 320
obsidian_furnace.generation = 500
crystal_furnace.generation = 360
netherite_furnace.generation = 1000
copper_furnace.generation = 40
silver_furnace.generation = 100
rainbow_furnace.generation = 2000
allthemodium_furnace.generation = 2000
vibranium_furnace.generation = 3000
unobtainium_furnace.generation = 5000
rainbow_furnace.rainbow_generation = 50000
recipeMaxXPLevel.level = 100
copper_furnace.tier = 0
iron_furnace.tier = 0
silver_furnace.tier = 1
gold_furnace.tier = 1
diamond_furnace.tier = 1
emerald_furnace.tier = 1
crystal_furnace.tier = 2
obsidian_furnace.tier = 2
netherite_furnace.tier = 2
rainbow_furnace.tier = 2
allthemodium_furnace.tier = 2
vibranium_furnace.tier = 2
unobtainium_furnace.tier = 2
";

fn values(host: &dyn Host) -> &'static HashMap<String, i32> {
    static VALUES: OnceLock<HashMap<String, i32>> = OnceLock::new();
    VALUES.get_or_init(|| {
        let mut values = parse(DEFAULT);
        values.extend(parse(&read(host)));
        values
    })
}

fn read(host: &dyn Host) -> String {
    let path = host.config_dir().join(FILE);
    match std::fs::read(&path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, DEFAULT);
            String::new()
        }
    }
}

pub(crate) fn parse(text: &str) -> HashMap<String, i32> {
    let mut out = HashMap::new();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim();
        let n = match v {
            "true" => 1,
            "false" => 0,
            _ => match v.parse() {
                Ok(n) => n,
                Err(_) => continue,
            },
        };
        out.insert(k.trim().to_owned(), n);
    }
    out
}

pub(crate) fn cfg(host: &dyn Host, key: &str) -> i32 {
    values(host).get(key).copied().unwrap_or(0)
}

/// A furnace block's config name (the Rainbow Furnace's block is `million_furnace`).
pub(crate) fn name_of(block: &str) -> &str {
    let name = block.strip_prefix("ironfurnaces:").unwrap_or(block);
    if name == "million_furnace" { "rainbow_furnace" } else { name }
}

pub(crate) fn speed(host: &dyn Host, block: &str) -> i32 {
    cfg(host, &format!("{}.speed", name_of(block)))
}

pub(crate) fn tier(host: &dyn Host, block: &str) -> i32 {
    cfg(host, &format!("{}.tier", name_of(block)))
}

pub(crate) fn generation(host: &dyn Host, block: &str) -> i32 {
    cfg(host, &format!("{}.generation", name_of(block)))
}

/// getCapacityFromTier.
pub(crate) fn capacity(host: &dyn Host, block: &str) -> i32 {
    match tier(host, block) {
        1 => cfg(host, "energy.tier_1"),
        2 => cfg(host, "energy.tier_2"),
        _ => cfg(host, "energy.tier_0"),
    }
}

/// How many a furnace smelts at once: the Rainbow's 64, the mult of Allthemodium, Vibranium and Unobtainium.
pub(crate) fn smelt_mult(host: &dyn Host, block: &str) -> i32 {
    match block {
        "ironfurnaces:million_furnace" => 64,
        "ironfurnaces:allthemodium_furnace" | "ironfurnaces:vibranium_furnace" | "ironfurnaces:unobtainium_furnace" => {
            cfg(host, &format!("{}.mult", name_of(block))).clamp(1, 64)
        }
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn defaults_parse() {
        let v = super::parse(super::DEFAULT);
        assert_eq!(v["iron_furnace.speed"], 160);
        assert_eq!(v["misc.lightupdates"], 0);
        assert_eq!(v["unobtainium_furnace.tier"], 2);
    }
}
