//! BlockIronFurnaceTileBase: a furnace's tick as a furnace, a factory and a generator, its augments, side settings,
//! auto input and output, redstone modes, experience, and the Rainbow Furnace's generation. Server only: it runs in
//! the block entity's tick, with the entity lent to it (out of the world meanwhile).

use crate::config::{self, cfg};
use crate::recipes::{self, Cooked, Kind};
use crate::util::*;
use crate::{owners, with_state};
use mod_api::*;

pub(crate) type Fields = mod_api::item::containers::ModBlockEntityFields;

pub(crate) const INPUT: usize = 0;
pub(crate) const FUEL: usize = 1;
pub(crate) const OUTPUT: usize = 2;
pub(crate) const AUGMENT_RED: usize = 3;
pub(crate) const AUGMENT_GREEN: usize = 4;
pub(crate) const AUGMENT_BLUE: usize = 5;
pub(crate) const GENERATOR_FUEL: usize = 6;
pub(crate) const FACTORY_INPUT: [usize; 6] = [7, 8, 9, 10, 11, 12];
pub(crate) const SLOTS: usize = 19;

// The menu's data slots, read by the screen (ContainerData).
pub(crate) const D_BURN: usize = 0;
pub(crate) const D_BURN_TOTAL: usize = 1;
pub(crate) const D_COOK: usize = 2;
pub(crate) const D_COOK_TOTAL: usize = 3;
pub(crate) const D_GEN: usize = 4;
pub(crate) const D_GEN_TOTAL: usize = 5;
pub(crate) const D_ENERGY: usize = 6;
pub(crate) const D_CAPACITY: usize = 7;
pub(crate) const D_FACTORY_COOK: usize = 8;
pub(crate) const D_FACTORY_TOTAL: usize = 14;
pub(crate) const D_SIDES: usize = 20; // bottom, top, front, back, left, right
pub(crate) const D_AUTO_IN: usize = 26;
pub(crate) const D_AUTO_OUT: usize = 27;
pub(crate) const D_REDSTONE: usize = 28;
pub(crate) const D_AUGMENT_GUI: usize = 29;
pub(crate) const D_SPLIT: usize = 30;
pub(crate) const D_MODE: usize = 31; // 0 furnace, 1 factory, 2 generator
pub(crate) const D_TIER: usize = 32;
pub(crate) const D_RAINBOW: usize = 33;
pub(crate) const D_ENERGY_HI: usize = 34;
pub(crate) const D_CAPACITY_HI: usize = 35;
pub(crate) const DATA_SLOTS: usize = 36;

/// A menu's data slot carries 16 bits: a number past that goes in two.
pub(crate) fn wide(data: &[i32], lo: usize, hi: usize) -> i32 {
    (data[lo] & 0xFFFF) | (data[hi] << 16)
}

pub(crate) fn is_furnace(block: &str) -> bool {
    block.strip_prefix("ironfurnaces:").is_some_and(|n| n.ends_with("_furnace"))
}

pub(crate) fn is_heater(block: &str) -> bool {
    block == "ironfurnaces:heater"
}

pub(crate) fn is_rainbow(block: &str) -> bool {
    block == "ironfurnaces:million_furnace"
}

pub(crate) const FURNACES: [&str; 13] = [
    "ironfurnaces:copper_furnace", "ironfurnaces:iron_furnace", "ironfurnaces:silver_furnace", "ironfurnaces:gold_furnace",
    "ironfurnaces:diamond_furnace", "ironfurnaces:emerald_furnace", "ironfurnaces:crystal_furnace",
    "ironfurnaces:obsidian_furnace", "ironfurnaces:netherite_furnace", "ironfurnaces:allthemodium_furnace",
    "ironfurnaces:vibranium_furnace", "ironfurnaces:unobtainium_furnace", "ironfurnaces:million_furnace",
];

// ---- What an item is

pub(crate) fn is_heater_item(id: &str) -> bool {
    id == "ironfurnaces:item_heater"
}

pub(crate) fn augment_slot(id: &str) -> Option<usize> {
    match id {
        "ironfurnaces:augment_blasting" | "ironfurnaces:augment_smoking" => Some(AUGMENT_RED),
        "ironfurnaces:augment_speed" | "ironfurnaces:augment_fuel" => Some(AUGMENT_GREEN),
        "ironfurnaces:augment_factory" | "ironfurnaces:augment_generator" => Some(AUGMENT_BLUE),
        _ => None,
    }
}

/// getAugment: 1 for blasting, speed and factory; 2 for smoking, fuel and generator.
fn augment_value(host: &dyn Host, stack: &Option<ItemStack>) -> i32 {
    match stack.as_ref().map(|s| id_of(host, s)).as_deref() {
        Some("ironfurnaces:augment_blasting" | "ironfurnaces:augment_speed" | "ironfurnaces:augment_factory") => 1,
        Some("ironfurnaces:augment_smoking" | "ironfurnaces:augment_fuel" | "ironfurnaces:augment_generator") => 2,
        _ => 0,
    }
}

/// 0 furnace, 1 factory, 2 generator: the blue augment.
pub(crate) fn mode(host: &dyn Host, e: &Fields) -> i32 {
    e.items.get(AUGMENT_BLUE).map_or(0, |s| augment_value(host, s))
}

pub(crate) fn recipe_kind(host: &dyn Host, e: &Fields) -> Kind {
    match e.items.get(AUGMENT_RED).and_then(|s| s.as_ref()).map(|s| id_of(host, s)).as_deref() {
        Some("ironfurnaces:augment_smoking") => Kind::Smoking,
        Some("ironfurnaces:augment_blasting") => Kind::Blasting,
        _ => Kind::Smelting,
    }
}

fn green(host: &dyn Host, e: &Fields, id: &str) -> bool {
    e.items.get(AUGMENT_GREEN).is_some_and(|s| is(host, s, id))
}

pub(crate) fn cook_recipe(host: &dyn Host, level: LevelRef, e: &Fields, stack: &ItemStack) -> Option<Cooked> {
    recipes::cook(host, level, recipe_kind(host, e), stack)
}

/// getBurnTime: the fuel's burn time (ItemRainbowCoal's own 200).
pub(crate) fn burn_time(host: &dyn Host, stack: &ItemStack) -> i32 {
    if id_of(host, stack) == "ironfurnaces:rainbow_coal" {
        return 200;
    }
    host.item_burn_time(stack.item()).unwrap_or(0).max(0)
}

pub(crate) fn is_fuel(host: &dyn Host, stack: &ItemStack) -> bool {
    burn_time(host, stack) > 0 || is_heater_item(&id_of(host, stack))
}

/// getSmokingBurnTime: a food's nutrition times 800.
pub(crate) fn smoking_burn(host: &dyn Host, stack: &ItemStack) -> i32 {
    match host.item_default_component(stack.item(), "minecraft:food") {
        Some(Tag::Compound(food)) => get_i(&food, "nutrition", 0).max(0) * 800,
        _ => 0,
    }
}

/// getSpeed: the tier's time, scaled by the recipe's own against 200.
fn speed_for(host: &dyn Host, block: &str, recipe: &Cooked) -> i32 {
    let regular = config::speed(host, block);
    ((f64::from(regular) / (200.0 / f64::from(recipe.time.max(1)))) as i32).max(1)
}

/// getCookTime / getFactoryCookTime: the ticks one smelt of what's in `slot` takes now; `current` when it's empty.
fn cook_time(host: &dyn Host, level: LevelRef, block: &str, e: &Fields, slot: usize, current: i32) -> i32 {
    match e.items[slot].as_ref() {
        None => current,
        Some(stack) => cook_time_of(host, level, block, e, stack),
    }
}

pub(crate) fn cook_time_of(host: &dyn Host, level: LevelRef, block: &str, e: &Fields, stack: &ItemStack) -> i32 {
    let Some(recipe) = cook_recipe(host, level, e, stack) else { return 0 };
    let mut speed = speed_for(host, block, &recipe);
    if green(host, e, "ironfurnaces:augment_speed") {
        speed = (speed / 2).max(1);
    }
    if green(host, e, "ironfurnaces:augment_fuel") {
        speed = ((f64::from(speed) * 1.25).ceil() as i32).max(1);
    }
    speed.max(1)
}

/// getGeneration.
fn generation(host: &dyn Host, block: &str, e: &Fields) -> i32 {
    let rf = config::generation(host, block);
    if green(host, e, "ironfurnaces:augment_speed") {
        rf * 2
    } else if green(host, e, "ironfurnaces:augment_fuel") {
        (f64::from(rf) * 0.75) as i32
    } else {
        rf
    }
}

/// getGeneratorBurn and isItemGeneratorFuel.
pub(crate) fn generator_burn_of(host: &dyn Host, e: &Fields, stack: &ItemStack) -> i32 {
    let mut burn = match e.items[AUGMENT_RED].as_ref().map(|s| id_of(host, s)).as_deref() {
        Some("ironfurnaces:augment_smoking") => smoking_burn(host, stack),
        Some("ironfurnaces:augment_blasting") => recipes::generator_energy(host, stack).map_or(0, |e| e / 20),
        _ => burn_time(host, stack),
    };
    if green(host, e, "ironfurnaces:augment_speed") {
        burn /= 2;
    } else if green(host, e, "ironfurnaces:augment_fuel") {
        burn *= 2;
    }
    burn
}

/// The factory's rows by tier: two, four or six.
pub(crate) fn factory_range(tier: i32) -> std::ops::Range<usize> {
    match tier {
        0 => 2..4,
        1 => 1..5,
        _ => 0..6,
    }
}

/// How many smelts fit at once: `per` made each, `have` already in a slot that holds `limit`, up to `mult` (1 for a
/// plain furnace) and the `inputs` there are. An empty slot (`have` 0) holds `limit` too.
pub(crate) fn smelt_times(limit: u32, have: u32, per: u32, mult: u32, inputs: u32) -> u32 {
    (limit.saturating_sub(have) / per.max(1)).min(mult.max(1)).min(inputs)
}

/// canSmelt / canFactorySmelt.
fn can_smelt(host: &dyn Host, recipe: Option<&Cooked>, e: &Fields, output: usize) -> bool {
    let Some(recipe) = recipe else { return false };
    let limit = max_stack(host, &recipe.result);
    match e.items[output].as_ref() {
        None => smelt_times(limit, 0, recipe.result.count(), 1, 1) == 1,
        Some(out) => out.is_stackable_with(&recipe.result) && smelt_times(limit, out.count(), recipe.result.count(), 1, 1) == 1,
    }
}

/// smeltItem / smeltItemMult / the factory's: what's made, kept for its experience (setRecipeUsed).
fn smelt(host: &dyn Host, block: &str, recipe: &Cooked, e: &mut Fields, input: usize, output: usize, wet_sponge: bool) {
    let Some(stack) = e.items[input].clone() else { return };
    let mult = config::smelt_mult(host, block) as u32;
    let have = e.items[output].as_ref().map_or(0, |s| s.count());
    let per = recipe.result.count();
    let times = smelt_times(max_stack(host, &recipe.result), have, per, mult, stack.count());
    if times == 0 {
        return;
    }
    match e.items[output].as_mut() {
        Some(out) => {
            let _ = out.grow(per * times);
        }
        None => e.items[output] = with_count(&recipe.result, per * times),
    }
    add_recipe_used(&mut e.nbt, &recipe.id, recipe.experience, times);
    if wet_sponge && id_of(host, &stack) == "minecraft:wet_sponge" && is(host, &e.items[FUEL], "minecraft:bucket") {
        e.items[FUEL] = stack_of(host, "minecraft:water_bucket", 1);
    }
    shrink(&mut e.items[input], times);
}

/// RecipesUsed: the crafted count and experience, by recipe, until someone takes the result.
fn add_recipe_used(nbt: &mut Compound, id: &str, experience: f32, times: u32) {
    let key = java(&format!("RecipesUsed.{id}"));
    let count = match nbt.get(&key) {
        Some(Tag::Compound(c)) => get_i(c, "n", 0),
        _ => 0,
    };
    let mut entry = Compound::new();
    set_i(&mut entry, "n", count + times as i32);
    set_f(&mut entry, "xp", f64::from(experience));
    nbt.insert(key, Tag::Compound(entry));
}

/// The recipes kept for experience, and the experience: splitAndSpawnExperience's amount over them. Clears them.
pub(crate) fn take_experience(nbt: &mut Compound) -> (Vec<String>, u32) {
    let mut keys: Vec<JavaString> = nbt
        .keys()
        .filter(|k| k.to_string_lossy().starts_with("RecipesUsed."))
        .cloned()
        .collect();
    // Keep the recipe award order when Compound traversal is not sorted.
    keys.sort_unstable();
    let mut total = 0u32;
    let mut ids = Vec::new();
    for key in keys {
        if let Some(Tag::Compound(c)) = nbt.remove(&key) {
            ids.push(key.to_string_lossy().trim_start_matches("RecipesUsed.").to_owned());
            let amount = f64::from(get_i(&c, "n", 0)) * get_f(&c, "xp");
            total += amount.floor().max(0.0) as u32;
            if roll() < amount.fract() {
                total += 1;
            }
        }
    }
    (ids, total)
}

/// What's kept for experience, without taking it (a broken furnace's, from its saved compound).
pub(crate) fn stored_experience(nbt: &Compound) -> u32 {
    take_experience(&mut nbt.clone()).1
}

fn roll() -> f64 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    f64::from(nanos % 1000) / 1000.0
}

// ---- Neighbours: items and energy through their capabilities

/// autoIO's input half: from the neighbour at `dir` into `slot`, what `accept` takes.
fn pull_into(
    host: &mut dyn ServerHost,
    level: LevelRef,
    e: &mut Fields,
    pos: Pos,
    dir: usize,
    slot: usize,
    accept: &dyn Fn(&dyn Host, &Fields, &ItemStack) -> bool,
) {
    let there = offset(pos, dir);
    let Some(other) = level.capability(host, &ITEMS, there, Some(DIRECTIONS[opposite(dir)])) else { return };
    let size = other.size(host);
    for i in 0..size {
        let Some(resource) = other.resource(host, i) else { continue };
        let available = other.amount(host, i);
        let Some(stack) = resource.to_stack(available.max(1)) else { continue };
        let room = match e.items[slot].as_ref() {
            Some(have) if !have.is_stackable_with(&stack) => continue,
            Some(have) => max_stack(host, have).saturating_sub(have.count()),
            None => {
                if !accept(host, e, &stack) {
                    continue;
                }
                max_stack(host, &stack)
            }
        };
        if room == 0 {
            return;
        }
        let want = room.min(available);
        let got = host
            .transaction(|host, tx| other.extract_at(host, tx, i, &resource, want))
            .unwrap_or(0);
        if got == 0 {
            continue;
        }
        match e.items[slot].as_mut() {
            Some(have) => {
                let _ = have.grow(got);
            }
            None => e.items[slot] = resource.to_stack(got),
        }
    }
}

/// autoIO's output half: from `slot` into the neighbour at `dir`.
fn push_from(host: &mut dyn ServerHost, level: LevelRef, e: &mut Fields, pos: Pos, dir: usize, slot: usize) {
    let Some(stack) = e.items[slot].clone() else { return };
    let there = offset(pos, dir);
    let Some(other) = level.capability(host, &ITEMS, there, Some(DIRECTIONS[opposite(dir)])) else { return };
    let resource = ItemResource::of(&stack);
    let went = host
        .transaction(|host, tx| Ok::<u32, ()>(other.insert(host, tx, &resource, stack.count()).unwrap_or(0)))
        .unwrap_or(0)
        .min(stack.count());
    shrink(&mut e.items[slot], went);
}

fn has_cook_recipe(host: &dyn Host, level: LevelRef, e: &Fields, stack: &ItemStack) -> bool {
    cook_recipe(host, level, e, stack).is_some()
}

/// FurnaceSettings: the six sides by Direction id (down, up, north, south, west, east), 0 none, 1 input, 2 output,
/// 3 input and output, 4 fuel.
pub(crate) fn side(nbt: &Compound, i: usize) -> i32 {
    int_array(nbt, "Settings", 6)[i]
}

pub(crate) fn auto_io(nbt: &Compound, i: usize) -> bool {
    int_array(nbt, "AutoIO", 2)[i] != 0
}

/// autoIO, autoIOGenerator and autoFactoryIO.
fn auto_transfer(host: &mut dyn ServerHost, level: LevelRef, block: &str, e: &mut Fields, pos: Pos) {
    let (auto_in, auto_out) = (auto_io(&e.nbt, 0), auto_io(&e.nbt, 1));
    if !auto_in && !auto_out {
        return;
    }
    let m = mode(host, e);
    for dir in 0..6 {
        let setting = side(&e.nbt, dir);
        if setting == 0 {
            continue;
        }
        match m {
            0 => {
                if auto_in && matches!(setting, 1 | 3) {
                    pull_into(host, level, e, pos, dir, INPUT, &|h, e, s| has_cook_recipe(h, level, e, s));
                }
                if auto_in && setting == 4 {
                    pull_into(host, level, e, pos, dir, FUEL, &|h, _, s| is_fuel(h, s));
                }
                if auto_out && setting == 4 && e.items[FUEL].as_ref().is_some_and(|f| !is_fuel(host, f)) {
                    push_from(host, level, e, pos, dir, FUEL);
                }
                if auto_out && matches!(setting, 2 | 3) {
                    push_from(host, level, e, pos, dir, OUTPUT);
                }
            }
            1 => {
                let range = factory_range(config::tier(host, block));
                if auto_in && matches!(setting, 1 | 3) {
                    for j in range.clone() {
                        pull_into(host, level, e, pos, dir, FACTORY_INPUT[j], &|h, e, s| has_cook_recipe(h, level, e, s));
                    }
                }
                if auto_out && matches!(setting, 2 | 3) {
                    for j in range {
                        push_from(host, level, e, pos, dir, FACTORY_INPUT[j] + 6);
                    }
                }
            }
            _ => {
                if setting != 4 {
                    continue;
                }
                if auto_in {
                    pull_into(host, level, e, pos, dir, GENERATOR_FUEL, &|h, e, s| {
                        let id = id_of(h, s);
                        id != "minecraft:bucket" && !is_heater_item(&id) && generator_burn_of(h, e, s) > 0
                    });
                }
                if auto_out && e.items[GENERATOR_FUEL].as_ref().is_some_and(|f| generator_burn_of(host, e, f) <= 0) {
                    push_from(host, level, e, pos, dir, GENERATOR_FUEL);
                }
            }
        }
    }
}

pub(crate) fn energy(e: &Fields) -> i32 {
    get_i(&e.nbt, "energy", 0)
}

fn set_energy(host: &dyn Host, block: &str, e: &mut Fields, value: i32) {
    let c = config::capacity(host, block);
    set_i(&mut e.nbt, "energy", value.clamp(0, c));
}

/// Puts up to `amount` into the energy handler of the block at `dir`, as that side reaches it: how much went.
fn energy_to(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, dir: usize, amount: i32) -> i32 {
    if amount <= 0 {
        return 0;
    }
    let Some(other) = level.capability(host, &ENERGY, offset(pos, dir), Some(DIRECTIONS[opposite(dir)])) else { return 0 };
    host.transaction(|host, tx| other.insert(host, tx, amount as u32))
        .unwrap_or(0)
        .min(amount as u32) as i32
}

/// Whether the block at `dir` has room for energy (energyHandlerCanAccept and amount < capacity).
fn energy_room(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, dir: usize) -> bool {
    let Some(other) = level.capability(host, &ENERGY, offset(pos, dir), Some(DIRECTIONS[opposite(dir)])) else { return false };
    other.amount(host) < other.capacity(host)
}

/// energyOut: to each side set to output whose neighbour takes energy, as much as it takes.
fn energy_out(host: &mut dyn ServerHost, level: LevelRef, block: &str, e: &mut Fields, pos: Pos) {
    let sides: Vec<usize> = (0..6).filter(|&d| matches!(side(&e.nbt, d), 2 | 3)).filter(|&d| energy_room(host, level, pos, d)).collect();
    for dir in sides {
        let have = energy(e);
        if have <= 0 {
            return;
        }
        let moved = energy_to(host, level, pos, dir, have);
        set_energy(host, block, e, have - moved);
    }
}

fn set_prop(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, key: &str, value: &str) {
    if level.block_property(host, pos, key).as_deref() != Some(value) {
        host.set_block_property_with(pos, key, value, 3);
    }
}

// ---- The tick

/// BlockIronFurnaceTileBase.tick.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: Pos, block: &str, e: &mut Fields) {
    let Some(level) = host.level(host.dimension()) else { return };
    recipes::ensure_sets(host);
    if e.items.len() < SLOTS {
        e.items.resize(SLOTS, None);
    }
    if e.data.len() < DATA_SLOTS {
        e.data.resize(DATA_SLOTS, 0);
    }
    first_tick(host, level, pos, block, e);
    let time = level.game_time(host).unwrap_or(0);
    let was_burning = get_i(&e.nbt, "BurnTime", 0) > 0;
    // currentAugment: a change restarts the burn; a change of the blue one, or of a generator's red, drops the contents.
    let mut current = int_array(&e.nbt, "Augment", 3);
    for i in 0..3 {
        let value = augment_value(host, &e.items[AUGMENT_RED + i]);
        if current[i] != value {
            let generator = current[2] == 2;
            current[i] = value;
            set_i(&mut e.nbt, "BurnTime", 0);
            set_f(&mut e.nbt, "GeneratorBurn", 0.0);
            if i == 2 || (generator && i == 0) {
                drop_contents(host, level, pos, e);
            }
        }
    }
    set_int_array(&mut e.nbt, "Augment", current);
    let m = mode(host, e);
    // The client's copy: what its screen shows and its particles (block_entity_update_tag).
    if get_i(&e.nbt, "hz_mode", -1) != m {
        set_i(&mut e.nbt, "hz_mode", m);
        host.block_entity_sync(pos);
    }
    if is_rainbow(block) {
        rainbow_tick(host, level, pos, e, m);
    }
    let total = get_i(&e.nbt, "CookTimeTotal", 0);
    let total = cook_time(host, level, block, e, INPUT, total);
    set_i(&mut e.nbt, "CookTimeTotal", total);
    let stopped = match get_i(&e.nbt, "Redstone", 0) {
        1 => level.has_neighbor_signal(host, pos),
        2 => !level.has_neighbor_signal(host, pos),
        _ => false,
    };
    if stopped {
        set_i(&mut e.nbt, "CookTime", 0);
        set_i(&mut e.nbt, "BurnTime", 0);
        set_prop(host, level, pos, "lit", "false");
    } else {
        match m {
            1 => factory_tick(host, level, pos, block, e, time),
            2 => generator_tick(host, level, pos, block, e, time),
            _ => furnace_tick(host, level, pos, block, e, time, was_burning),
        }
    }
    if time % 24 == 0 {
        let kind = match recipe_kind(host, e) {
            Kind::Smoking => "1",
            Kind::Blasting => "2",
            Kind::Smelting => "0",
        };
        set_prop(host, level, pos, "type", kind);
        let jovial = get_i(&e.nbt, "Jovial", 0).clamp(0, 2).to_string();
        set_prop(host, level, pos, "jovial", &jovial);
    }
    let facing = level.block_property(host, pos, "facing").unwrap_or_else(|| "north".to_owned());
    fill_data(host, block, e, mode(host, e), &facing);
}

/// setPlacedBy and placeConfig, the first time the entity ticks: the sides (bottom output, top input, the rest fuel),
/// the cook time, and, for whoever placed it, their list of furnaces and the Rainbow Furnace's owner.
fn first_tick(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, block: &str, e: &mut Fields) {
    if e.nbt.get(&java("hz_placed")).is_some() {
        return;
    }
    set_i(&mut e.nbt, "hz_placed", 1);
    if e.nbt.get(&java("Settings")).is_none() {
        let mut sides = vec![4; 6];
        sides[0] = 2;
        sides[1] = 1;
        set_int_array(&mut e.nbt, "Settings", sides);
    }
    set_i(&mut e.nbt, "CookTimeTotal", config::speed(host, block));
    let placer = with_state(|s| s.placed.remove(&(host.dimension().to_owned(), pos)));
    if let Some(uuid) = placer {
        owners::add(host, uuid, pos);
        if is_rainbow(block) {
            set_owner(&mut e.nbt, uuid);
        }
    }
    let _ = level;
}

fn furnace_tick(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, block: &str, e: &mut Fields, time: i64, was_burning: bool) {
    let mut burn = get_i(&e.nbt, "BurnTime", 0);
    let mut cook = get_i(&e.nbt, "CookTime", 0);
    let total = get_i(&e.nbt, "CookTimeTotal", 200);
    if burn > 0 {
        burn -= 1;
    }
    let has_fuel = e.items[FUEL].is_some();
    if burn > 0 || (has_fuel && e.items[INPUT].is_some()) {
        let recipe = e.items[INPUT].as_ref().and_then(|s| cook_recipe(host, level, e, s));
        let valid = can_smelt(host, recipe.as_ref(), e, OUTPUT);
        if burn <= 0 && valid {
            burn = consume_fuel(host, level, e, total);
        }
        if burn > 0 && valid {
            cook += 1;
            if cook >= total {
                cook = 0;
                let next = cook_time(host, level, block, e, INPUT, total);
                set_i(&mut e.nbt, "CookTimeTotal", next);
                if let Some(recipe) = recipe.as_ref() {
                    smelt(host, block, recipe, e, INPUT, OUTPUT, true);
                }
                auto_transfer(host, level, block, e, pos);
            }
        } else {
            cook = 0;
        }
    } else if cook > 0 {
        cook = (cook - 2).clamp(0, total);
    }
    set_i(&mut e.nbt, "BurnTime", burn);
    set_i(&mut e.nbt, "CookTime", cook);
    if time % 24 == 0 && cook <= 0 {
        auto_transfer(host, level, block, e, pos);
    }
    if was_burning != (burn > 0) {
        set_prop(host, level, pos, "lit", if burn > 0 { "true" } else { "false" });
    }
}

/// The furnace's fuel, or the Wireless Heat Transmitter's power through its receiver: how long it now burns.
fn consume_fuel(host: &mut dyn ServerHost, level: LevelRef, e: &mut Fields, total: i32) -> i32 {
    let Some(fuel) = e.items[FUEL].clone() else { return 0 };
    let fuel_augment = green(host, e, "ironfurnaces:augment_fuel");
    let speed_augment = green(host, e, "ironfurnaces:augment_speed");
    let fuel_id = id_of(host, &fuel);
    if is_heater_item(&fuel_id) {
        let Some(bound) = heater_position(&fuel) else { return 0 };
        let Some(heater) = level.block(host, bound).filter(|b| b.id(host).is_some_and(is_heater)).and_then(|_| host.block_entity(level, bound)) else {
            return 0;
        };
        let stored = heater.data(host).map_or(0, |d| get_i(&d, "energy", 0));
        if stored < 2000 {
            return 0;
        }
        let burn = if fuel_augment {
            400 * total / 200
        } else if !speed_augment {
            200 * total / 200
        } else if stored >= 4000 {
            100 * total / 200
        } else {
            0
        };
        if burn > 0 {
            // removeEnergy(2000): the transmitter's own store, as the Java mod takes it.
            let mut left = Compound::new();
            set_i(&mut left, "energy", stored - 2000);
            let _ = heater.merge_data(host, left);
        }
        set_i(&mut e.nbt, "RecipesUsed", burn);
        return burn;
    }
    let time = burn_time(host, &fuel);
    let burn = if fuel_augment {
        time * total / 200 * 2
    } else if speed_augment {
        time * total / 200 / 2
    } else {
        time * total / 200
    };
    set_i(&mut e.nbt, "RecipesUsed", burn);
    if burn > 0 && fuel_id == "ironfurnaces:rainbow_coal" {
        // ItemRainbowCoal.getCraftingRemainder: itself, a use worn, 5120 uses in all.
        let damage = fuel.damage() + 1;
        if damage >= 5120 {
            e.items[FUEL] = None;
        } else if let Some(f) = e.items[FUEL].as_mut() {
            f.set_damage(damage);
        }
        return burn;
    }
    if burn > 0 {
        match host.item_crafting_remainder(&fuel) {
            Some(rest) => e.items[FUEL] = Some(rest),
            None => shrink(&mut e.items[FUEL], 1),
        }
    }
    burn
}

fn factory_tick(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, block: &str, e: &mut Fields, time: i64) {
    let range = factory_range(config::tier(host, block));
    let split = get_i(&e.nbt, "AutoSplit", 0) != 0;
    if split {
        auto_split(host, block, e, range.clone(), false);
    }
    let mut cook = int_array(&e.nbt, "FactoryCookTime", 6);
    let mut totals = int_array(&e.nbt, "FactoryTotalCookTime", 6);
    for j in range.clone() {
        let slot = FACTORY_INPUT[j];
        totals[j] = cook_time(host, level, block, e, slot, totals[j]);
        let used_key = format!("UsedRF{j}");
        let mut used = get_f(&e.nbt, &used_key);
        if let Some(stack) = e.items[slot].clone() {
            let recipe = cook_recipe(host, level, e, &stack);
            if can_smelt(host, recipe.as_ref(), e, slot + 6)
                && let Some(recipe) = recipe
            {
                let base = recipe.time as i32 * 20;
                let need = if green(host, e, "ironfurnaces:augment_speed") {
                    base * 2
                } else if green(host, e, "ironfurnaces:augment_fuel") {
                    base / 2
                } else {
                    base
                };
                if energy(e) >= need || cook[j] > 0 {
                    cook[j] += 1;
                    let step = f64::from(need) / f64::from(totals[j].max(1));
                    used += step;
                    set_energy(host, block, e, (f64::from(energy(e)) - step) as i32);
                    if cook[j] >= totals[j] {
                        cook[j] = 0;
                        if used < f64::from(need) {
                            set_energy(host, block, e, (f64::from(energy(e)) - (f64::from(need) - used)) as i32);
                        }
                        used = 0.0;
                        totals[j] = cook_time(host, level, block, e, slot, totals[j]);
                        if split {
                            auto_split(host, block, e, range.clone(), true);
                        }
                        smelt(host, block, &recipe, e, slot, slot + 6, false);
                        auto_transfer(host, level, block, e, pos);
                    }
                }
            }
        } else {
            cook[j] = 0;
        }
        set_f(&mut e.nbt, &used_key, used);
    }
    let lit = cook.iter().any(|c| *c > 0);
    set_prop(host, level, pos, "lit", if lit { "true" } else { "false" });
    let idle = cook.iter().any(|c| *c <= 0);
    set_int_array(&mut e.nbt, "FactoryCookTime", cook);
    set_int_array(&mut e.nbt, "FactoryTotalCookTime", totals);
    if time % 24 == 0 && idle {
        auto_transfer(host, level, block, e, pos);
    }
}

/// split, fillEmptySlots and getSplitCounts: the factory's inputs even each other out. `full` is `split(true, ..)`, after
/// a smelt; otherwise it splits only while a row is empty.
fn auto_split(host: &dyn Host, block: &str, e: &mut Fields, range: std::ops::Range<usize>, full: bool) {
    // canAutoSplitWork: not the Million, Allthemodium, Vibranium or Unobtainium furnaces.
    if config::smelt_mult(host, block) > 1 {
        return;
    }
    let slots: Vec<usize> = range.map(|j| FACTORY_INPUT[j]).collect();
    if !full && slots.iter().all(|&s| e.items[s].is_some()) {
        return;
    }
    // itemToCheck: the last row's stack.
    let Some(check) = slots.iter().rev().find_map(|&s| e.items[s].clone()) else { return };
    // fillEmptySlots.
    let mut amount = slots.iter().filter(|&&s| e.items[s].is_none()).count() as u32;
    if amount > 0 {
        for &j in &slots {
            let Some(stack) = e.items[j].clone() else { continue };
            if stack.count() > 1 && amount > 0 {
                if amount >= stack.count() {
                    amount = stack.count() - 1;
                }
                shrink(&mut e.items[j], amount);
                for &s in &slots {
                    if e.items[s].is_none() && amount > 0 {
                        e.items[s] = with_count(&stack, 1);
                        amount -= 1;
                    }
                }
                break;
            }
        }
    }
    // getSplitCounts over the rows holding that item, the odd ones to the first rows.
    let rows: Vec<usize> = slots.iter().copied().filter(|&s| e.items[s].as_ref().is_some_and(|i| i.same_item(&check))).collect();
    let n = rows.len() as u32;
    if n == 0 {
        return;
    }
    let sum: u32 = rows.iter().map(|&s| e.items[s].as_ref().map_or(0, |i| i.count())).sum();
    let (each, extra) = (sum / n, sum % n);
    for (i, &s) in rows.iter().enumerate() {
        let want = each + u32::from((i as u32) < extra);
        if let Some(stack) = e.items[s].as_mut()
            && stack.count() != want
            && want > 0
        {
            let _ = stack.set_count(want);
        }
    }
}

fn generator_tick(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, block: &str, e: &mut Fields, time: i64) {
    let capacity = config::capacity(host, block);
    let mut burn = get_f(&e.nbt, "GeneratorBurn");
    let mut recent = get_i(&e.nbt, "GeneratorRecent", 0);
    let mut gotten = get_f(&e.nbt, "GottenRF");
    if energy(e) < capacity {
        if let Some(fuel) = e.items[GENERATOR_FUEL].clone()
            && burn <= 0.0
        {
            let b = generator_burn_of(host, e, &fuel);
            if b > 0 {
                burn = f64::from(b);
                recent = b;
                match host.item_crafting_remainder(&fuel) {
                    Some(rest) => e.items[GENERATOR_FUEL] = Some(rest),
                    None => shrink(&mut e.items[GENERATOR_FUEL], 1),
                }
            }
        }
        set_prop(host, level, pos, "lit", if burn > 0.0 { "true" } else { "false" });
        if burn > 0.0 {
            let rf = generation(host, block, e);
            let max = f64::from(recent) * 20.0;
            gotten += f64::from(rf);
            set_energy(host, block, e, energy(e) + rf);
            if burn - f64::from(rf) / 20.0 <= 0.0 {
                if gotten + f64::from(rf) > max && gotten + f64::from(rf) < f64::from(capacity) {
                    let diff = (gotten + f64::from(rf) - max) as i32;
                    set_energy(host, block, e, energy(e) + rf - diff);
                }
                if gotten + f64::from(rf) < max {
                    let diff = (max - gotten + f64::from(rf)) as i32;
                    set_energy(host, block, e, energy(e) + rf + diff);
                }
                gotten = 0.0;
            }
            burn -= f64::from(rf) / 20.0;
            if burn <= 0.0 {
                auto_transfer(host, level, block, e, pos);
                burn = 0.0;
            }
        }
    }
    if burn <= 0.0 {
        burn = 0.0;
    }
    set_f(&mut e.nbt, "GeneratorBurn", burn);
    set_i(&mut e.nbt, "GeneratorRecent", recent);
    set_f(&mut e.nbt, "GottenRF", gotten);
    energy_out(host, level, block, e, pos);
    if time % 24 == 0 && burn <= 0.0 {
        auto_transfer(host, level, block, e, pos);
    }
}

/// The Rainbow Furnace as a generator: one of each tier placed by its owner, running as generators with room to fill,
/// and no second Rainbow generator of theirs. It then sends rainbow_generation out of its output sides, split over
/// the ones that take energy, outside its own buffer (rainbowEnergyOut).
fn rainbow_tick(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, e: &mut Fields, m: i32) {
    if m != 2 {
        set_prop(host, level, pos, "rainbow", "false");
        return;
    }
    let owner = owner_of(&e.nbt);
    let mut generating = owner.is_some();
    const TIERS: [&str; 9] = ["iron", "gold", "diamond", "emerald", "obsidian", "crystal", "netherite", "copper", "silver"];
    let mut running = [false; 9];
    let mut rainbows = 0;
    let mut rainbow_generators = 0;
    if let Some(owner) = owner
        && host.player(owner).is_some()
    {
        for p in owners::list(host, owner) {
            if p == pos {
                rainbows += 1;
                rainbow_generators += 1;
                continue;
            }
            let Some(id) = level.block(host, p).and_then(|b| b.id(host).map(str::to_owned)) else { continue };
            if !is_furnace(&id) {
                continue;
            }
            let Some(data) = host.block_entity(level, p).and_then(|b| b.data(host)) else { continue };
            if is_rainbow(&id) {
                rainbows += 1;
                if get_i(&data, "hz_mode", 0) == 2 {
                    rainbow_generators += 1;
                }
            }
            if let Some(i) = TIERS.iter().position(|t| id == format!("ironfurnaces:{t}_furnace"))
                && get_f(&data, "GeneratorBurn") > 0.0
                && get_i(&data, "energy", 0) < config::capacity(host, &id)
            {
                running[i] = true;
            }
        }
    }
    if rainbows > 1 && rainbow_generators > 1 {
        generating = false;
    }
    let generating = generating && running.iter().all(|r| *r);
    set_i(&mut e.nbt, "RainbowGen", i32::from(generating));
    set_prop(host, level, pos, "rainbow", if generating { "true" } else { "false" });
    if generating {
        let sides: Vec<usize> = (0..6)
            .filter(|&d| matches!(side(&e.nbt, d), 2 | 3))
            .filter(|&d| level.capability(host, &ENERGY, offset(pos, d), Some(DIRECTIONS[opposite(d)])).is_some())
            .collect();
        if sides.is_empty() {
            return;
        }
        let each = cfg(host, "rainbow_furnace.rainbow_generation") / sides.len() as i32;
        for dir in sides {
            energy_to(host, level, pos, dir, each);
        }
    }
}

pub(crate) fn owner_of(nbt: &Compound) -> Option<u128> {
    match nbt.get(&java("Owner")) {
        Some(Tag::IntArray(a)) if a.len() == 4 => {
            let w: Vec<u32> = a.iter().map(|v| *v as u32).collect();
            Some((u128::from(w[0]) << 96) | (u128::from(w[1]) << 64) | (u128::from(w[2]) << 32) | u128::from(w[3]))
        }
        _ => None,
    }
}

/// `Owner`, as UUIDUtil.CODEC writes it: four ints, most significant first.
pub(crate) fn set_owner(nbt: &mut Compound, uuid: u128) {
    let words = vec![(uuid >> 96) as u32 as i32, (uuid >> 64) as u32 as i32, (uuid >> 32) as u32 as i32, uuid as u32 as i32];
    nbt.insert(java("Owner"), Tag::IntArray(words));
}

/// grantStoredRecipeExperience for `player`: the recipes (unlockRecipes) and the experience, as orbs where they stand.
pub(crate) fn award(host: &mut dyn ServerHost, player: PlayerRef, ids: &[String], amount: u32) {
    if !ids.is_empty() {
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        let _ = player.award_recipes(host, &ids);
    }
    if amount > 0 {
        let orbs = player.level(host).zip(player.entity().position(host)).is_some_and(|(level, at)| level.award_experience(host, at[0], at[1], at[2], amount));
        if !orbs {
            let _ = player.give_experience_points(host, amount);
        }
    }
}

/// The receiver's bound transmitter, from its `HeaterPos` custom data.
pub(crate) fn heater_position(stack: &ItemStack) -> Option<Pos> {
    let c = custom_data(stack)?;
    c.get(&java("HeaterPosX"))?;
    Some([get_i(&c, "HeaterPosX", 0), get_i(&c, "HeaterPosY", 0), get_i(&c, "HeaterPosZ", 0)])
}

pub(crate) fn set_heater_position(stack: &mut ItemStack, pos: Pos) {
    if heater_position(stack) == Some(pos) {
        return;
    }
    let mut c = custom_data(stack).unwrap_or_default();
    set_i(&mut c, "HeaterPosX", pos[0]);
    set_i(&mut c, "HeaterPosY", pos[1]);
    set_i(&mut c, "HeaterPosZ", pos[2]);
    set_custom_data(stack, c);
}

/// BlockWirelessEnergyHeaterTile.tick: a receiver in its slot is bound to it.
pub(crate) fn heater_tick(pos: Pos, e: &mut Fields) {
    if e.items.is_empty() {
        e.items.resize(1, None);
    }
    if e.data.len() < 2 {
        e.data.resize(2, 0);
    }
    let stored = get_i(&e.nbt, "energy", 0);
    if e.data[0] != stored {
        e.data[0] = stored;
    }
    if e.data[1] != 1_000_000 {
        e.data[1] = 1_000_000;
    }
    if let Some(stack) = e.items[0].as_ref()
        && heater_position(stack) != Some(pos)
    {
        let mut bound = stack.clone();
        set_heater_position(&mut bound, pos);
        e.items[0] = Some(bound);
    }
}

/// dropContents: everything but the augments, where the furnace stands.
fn drop_contents(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, e: &mut Fields) {
    for i in 0..SLOTS {
        if (AUGMENT_RED..=AUGMENT_BLUE).contains(&i) {
            continue;
        }
        if let Some(stack) = e.items[i].take() {
            let _ = level.spawn_item(host, f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]), stack, 10);
        }
    }
}

/// Direction ids of front, back, left and right for a facing.
pub(crate) fn relative_sides(facing: &str) -> [usize; 4] {
    match facing {
        "south" => [3, 2, 4, 5],
        "west" => [4, 5, 2, 3],
        "east" => [5, 4, 3, 2],
        _ => [2, 3, 5, 4],
    }
}

/// The screen's side order (bottom, top, front, back, left, right) as Direction ids.
pub(crate) fn screen_sides(facing: &str) -> [usize; 6] {
    let r = relative_sides(facing);
    [0, 1, r[0], r[1], r[2], r[3]]
}

fn set_data(d: &mut [i32], i: usize, v: i32) {
    if d[i] != v {
        d[i] = v;
    }
}

/// The menu's ContainerData, from the entity.
fn fill_data(host: &dyn Host, block: &str, e: &mut Fields, m: i32, facing: &str) {
    let order = screen_sides(facing);
    let sides = int_array(&e.nbt, "Settings", 6);
    let auto = int_array(&e.nbt, "AutoIO", 2);
    let cook = int_array(&e.nbt, "FactoryCookTime", 6);
    let totals = int_array(&e.nbt, "FactoryTotalCookTime", 6);
    let nbt = e.nbt.clone();
    let (energy_now, cap) = (get_i(&nbt, "energy", 0), config::capacity(host, block));
    let tier = config::tier(host, block);
    let d = &mut e.data;
    set_data(d, D_BURN, get_i(&nbt, "BurnTime", 0));
    set_data(d, D_BURN_TOTAL, get_i(&nbt, "RecipesUsed", 0));
    set_data(d, D_COOK, get_i(&nbt, "CookTime", 0));
    set_data(d, D_COOK_TOTAL, get_i(&nbt, "CookTimeTotal", 0));
    set_data(d, D_GEN, get_f(&nbt, "GeneratorBurn") as i32);
    set_data(d, D_GEN_TOTAL, get_i(&nbt, "GeneratorRecent", 0));
    set_data(d, D_ENERGY, energy_now & 0xFFFF);
    set_data(d, D_ENERGY_HI, energy_now >> 16);
    set_data(d, D_CAPACITY, cap & 0xFFFF);
    set_data(d, D_CAPACITY_HI, cap >> 16);
    for j in 0..6 {
        set_data(d, D_FACTORY_COOK + j, cook[j]);
        set_data(d, D_FACTORY_TOTAL + j, totals[j]);
        set_data(d, D_SIDES + j, sides[order[j]]);
    }
    set_data(d, D_AUTO_IN, auto[0]);
    set_data(d, D_AUTO_OUT, auto[1]);
    set_data(d, D_REDSTONE, get_i(&nbt, "Redstone", 0));
    set_data(d, D_AUGMENT_GUI, get_i(&nbt, "AugmentGUI", 0));
    set_data(d, D_SPLIT, get_i(&nbt, "AutoSplit", 0));
    set_data(d, D_MODE, m);
    set_data(d, D_TIER, tier);
    set_data(d, D_RAINBOW, i32::from(is_rainbow(block)));
}

// ---- Who may put what where

/// IisItemValidForSlot, with the furnace's mode and tier.
pub(crate) fn valid_for_slot(host: &dyn Host, level: LevelRef, block: &str, e: &Fields, slot: usize, stack: &ItemStack) -> bool {
    let id = id_of(host, stack);
    match mode(host, e) {
        0 => match slot {
            INPUT => has_cook_recipe(host, level, e, stack),
            FUEL => burn_time(host, stack) > 0 || (id == "minecraft:bucket" && !is(host, &e.items[FUEL], "minecraft:bucket")) || is_heater_item(&id),
            _ => false,
        },
        1 => {
            let range = factory_range(config::tier(host, block));
            (7..=12).contains(&slot) && range.contains(&(slot - 7)) && has_cook_recipe(host, level, e, stack)
        }
        _ => slot == GENERATOR_FUEL && !is_heater_item(&id) && generator_burn_of(host, e, stack) > 0,
    }
}

/// IgetSlotsForFace: the slots one side's setting reaches.
pub(crate) fn slots_for_face(host: &dyn Host, e: &Fields, setting: i32) -> &'static [usize] {
    match mode(host, e) {
        0 => match setting {
            1 => &[0, 1],
            2 => &[2],
            3 => &[0, 1, 2],
            4 => &[1],
            _ => &[],
        },
        2 => match setting {
            4 => &[6],
            _ => &[],
        },
        _ => match setting {
            1 => &[7, 8, 9, 10, 11, 12],
            2 => &[13, 14, 15, 16, 17, 18],
            3 => &[7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18],
            _ => &[],
        },
    }
}

/// IcanExtractItem.
pub(crate) fn can_extract(host: &dyn Host, e: &Fields, slot: usize, stack: &ItemStack, setting: i32) -> bool {
    let bucket = id_of(host, stack) == "minecraft:bucket";
    match mode(host, e) {
        0 => match setting {
            2 | 3 => slot == OUTPUT,
            4 => bucket,
            _ => false,
        },
        2 => setting == 4 && bucket,
        _ => matches!(setting, 2 | 3) && (13..=18).contains(&slot),
    }
}

/// getComparatorInputOverride's slots, for what the furnace is now.
pub(crate) fn comparator_slots(host: &dyn Host, block: &str, e: &Fields) -> Vec<usize> {
    let m = mode(host, e);
    let mut slots = Vec::new();
    if m == 0 {
        slots.push(INPUT);
    }
    slots.extend([FUEL, OUTPUT]);
    if m == 2 {
        slots.push(GENERATOR_FUEL);
    }
    if m == 1 {
        let t = config::tier(host, block);
        slots.extend([9, 10, 15, 16]);
        if t >= 1 {
            slots.extend([8, 11, 14, 17]);
        }
        if t >= 2 {
            slots.extend([7, 12, 13, 18]);
        }
    }
    slots
}

#[cfg(test)]
mod stack_limit_tests {
    use super::smelt_times;

    #[test]
    fn empty_output_slot_respects_a_low_limit() {
        assert_eq!(smelt_times(16, 0, 4, 4, 64), 4);
        assert_eq!(smelt_times(16, 0, 5, 4, 64), 3);
        assert_eq!(smelt_times(1, 0, 1, 4, 64), 1);
        assert_eq!(smelt_times(1, 0, 2, 1, 64), 0);
    }

    #[test]
    fn partly_filled_low_limit_stack_is_not_overfilled() {
        assert_eq!(smelt_times(16, 12, 1, 4, 64), 4);
        assert_eq!(smelt_times(16, 14, 1, 4, 64), 2);
        assert_eq!(smelt_times(16, 16, 1, 4, 64), 0);
        assert_eq!(smelt_times(16, 13, 4, 4, 64), 0);
        assert_eq!(smelt_times(1, 1, 1, 4, 64), 0);
    }

    #[test]
    fn plain_and_normal_cases_are_unchanged() {
        assert_eq!(smelt_times(64, 0, 1, 1, 64), 1);
        assert_eq!(smelt_times(64, 63, 1, 1, 64), 1);
        assert_eq!(smelt_times(64, 64, 1, 1, 64), 0);
        assert_eq!(smelt_times(64, 60, 1, 8, 64), 4);
        assert_eq!(smelt_times(64, 0, 1, 8, 3), 3);
        assert_eq!(smelt_times(16, 40, 1, 4, 64), 0);
    }
}
