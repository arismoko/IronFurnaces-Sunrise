//! Applied Energistics 2's crafting ETA (this port's addition): with AE2 installed, a furnace
//! tells it how long a craft of a processing pattern takes (`ae2::api::register_machine`), so a job's time left is right
//! from the first craft. The time is the furnace's own: its tier's speed against the recipe's time, the speed and fuel
//! augments (`getCookTime`), how many it smelts at once (the Rainbow, Allthemodium, Vibranium and Unobtainium mult),
//! and in factory mode the rows that run together (with Auto Split; without it a provider's pushes stack in the first
//! row). A generator, a furnace switched off by redstone or one without a cooking recipe for the inputs says nothing.
//! With the Machine Work library installed AE2 asks the furnaces' `machine_work::MACHINE_WORK` answer first (`work.rs`,
//! sharing `working_furnace` and `parallel_of` here); this stays for an AE2 without the library.

use crate::config;
use crate::furnace::{self, Fields, SLOTS};
use crate::util::*;
use mod_api::*;

/// A craft's time on the furnace: (ticks a craft on average, ticks until a craft's last smelt is out, ticks done on the
/// work under way). `cook`: ticks a smelt; `parallel`: smelts at once; `items`: smelts a craft (the pattern's cookable
/// inputs); `done`: the cook timer now (0 idle).
pub(crate) fn per_craft(cook: i32, parallel: u32, items: i64, done: i32) -> (f64, f64, Option<f64>) {
    let cook = f64::from(cook.max(1));
    let parallel = parallel.max(1) as f64;
    let items = items.max(1) as f64;
    let ticks = cook * items / parallel;
    let latency = cook * (items / parallel).ceil();
    (ticks, latency, (done > 0).then_some(f64::from(done)))
}

/// Smelts the furnace works at once: the mult, times the rows in factory mode when Auto Split shares the inputs out (it
/// doesn't for a furnace with a mult, `canAutoSplitWork`).
pub(crate) fn parallel(mode: i32, mult: i32, rows: usize, split: bool) -> u32 {
    let mult = mult.clamp(1, 64) as u32;
    if mode == 1 && split && mult == 1 { rows.max(1) as u32 } else { mult }
}

/// The furnace `block` at `pos` as its craft times read it: its fields (saved compound and slots) and its mode (0
/// furnace, 1 factory). None for no furnace, a generator, or a furnace switched off by redstone. Shared with the
/// `machine_work` capability's answer (`work.rs`).
#[cfg_attr(not(any(feature = "ae2", feature = "machine_work")), allow(dead_code))]
pub(crate) fn working_furnace(host: &dyn Host, level: LevelRef, pos: Pos, block: &str) -> Option<(Fields, i32)> {
    if !furnace::is_furnace(block) {
        return None;
    }
    let entity = host.block_entity(level, pos)?;
    let nbt = entity.data(host)?;
    let items = (0..SLOTS as u32).map(|i| entity.item_slot(host, i).ok().flatten()).collect();
    let e = Fields { nbt, items, data: Vec::new() };
    let mode = furnace::mode(host, &e);
    if mode == 2 {
        return None;
    }
    let stopped = match get_i(&e.nbt, "Redstone", 0) {
        1 => level.has_neighbor_signal(host, pos),
        2 => !level.has_neighbor_signal(host, pos),
        _ => false,
    };
    (!stopped).then_some((e, mode))
}

/// Smelts the furnace `block` with fields `e` in `mode` works at once (`parallel`): its mult, or its factory rows with
/// Auto Split.
#[cfg_attr(not(any(feature = "ae2", feature = "machine_work")), allow(dead_code))]
pub(crate) fn parallel_of(host: &dyn Host, block: &str, e: &Fields, mode: i32) -> u32 {
    let rows = if mode == 1 { furnace::factory_range(config::tier(host, block)).len() } else { 1 };
    let split = get_i(&e.nbt, "AutoSplit", 0) != 0;
    parallel(mode, config::smelt_mult(host, block), rows, split)
}

/// What the furnace at `pos` takes for one craft of `inputs` (item ids, and the amount a craft takes), as
/// `ae2::api::MachineProvider` asks: (ticks, latency, done).
#[cfg_attr(not(feature = "ae2"), allow(dead_code))]
pub(crate) fn furnace_time(host: &dyn Host, level: LevelRef, pos: Pos, block: &str, inputs: &[(&str, i64)]) -> Option<(f64, f64, Option<f64>)> {
    let (e, mode) = working_furnace(host, level, pos, block)?;
    // The inputs the furnace cooks (with its red augment's recipe type), and the smelt time of the first.
    let mut cook = 0;
    let mut smelts = 0i64;
    for (id, n) in inputs {
        let Some(stack) = stack_of(host, id, 1) else { continue };
        let t = furnace::cook_time_of(host, level, block, &e, &stack);
        if t > 0 {
            if cook == 0 {
                cook = t;
            }
            smelts += (*n).max(1);
        }
    }
    if cook == 0 {
        return None;
    }
    let range = furnace::factory_range(config::tier(host, block));
    let done = if mode == 1 {
        let timers = int_array(&e.nbt, "FactoryCookTime", 6);
        range.clone().filter_map(|j| timers.get(j).copied()).max().unwrap_or(0)
    } else {
        get_i(&e.nbt, "CookTime", 0)
    };
    Some(per_craft(cook, parallel_of(host, block, &e, mode), smelts, done))
}

/// `ae2::api::register_machine`'s provider.
#[cfg(feature = "ae2")]
pub(crate) fn machine_time(host: &dyn Host, level: LevelRef, pos: [i32; 3], block: &str, inputs: &[(&str, i64)]) -> Option<ae2::api::MachineTime> {
    furnace_time(host, level, pos, block, inputs).map(|(ticks, latency, done)| ae2::api::MachineTime { ticks, latency, done })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_furnace_smelts_one_at_a_time() {
        // An iron furnace (160) on a 200-tick recipe: 160 a craft of one ore; 8 ore a craft, 1280.
        assert_eq!(per_craft(160, 1, 1, 0), (160.0, 160.0, None));
        assert_eq!(per_craft(160, 1, 8, 40), (1280.0, 1280.0, Some(40.0)));
    }

    #[test]
    fn a_mult_or_factory_rows_work_together() {
        // Six rows with Auto Split: six ore of a craft at once.
        assert_eq!(parallel(1, 1, 6, true), 6);
        // Without it the pushes stack in one row.
        assert_eq!(parallel(1, 1, 6, false), 1);
        // A furnace with a mult never splits; its mult is how many at once.
        assert_eq!(parallel(1, 16, 6, true), 16);
        assert_eq!(parallel(0, 64, 1, false), 64);
        // Four at once, one ore a craft: 50 a craft on average, each out 200 after it went in.
        assert_eq!(per_craft(200, 4, 1, 0), (50.0, 200.0, None));
        // Six at once, 8 ore a craft: two batches for the craft's last.
        let (ticks, latency, _) = per_craft(100, 6, 8, 0);
        assert!((ticks - 800.0 / 6.0).abs() < 1e-9);
        assert_eq!(latency, 200.0);
    }
}
