//! What doesn't fit the other modules: animateTick's flames, smoke and sounds (client), and the comparator output
//! (server).

use crate::furnace::{self, Fields};
use crate::util::*;
use crate::{KEYS, with_state};
use mod_api::*;

/// A small random source for the client's effects (RandomSource in animateTick).
fn next(seed: &mut u64) -> f64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed >> 11) as f64 / (1u64 << 53) as f64
}

/// The client's tick: its count (a seed for the effects) and the recipe sets the screens ask for.
pub(crate) fn tick(host: &mut dyn ClientHost) {
    with_state(|s| s.client_ticks += 1);
    crate::recipes::ensure_sets(&*host);
}

/// BlockIronFurnaceBase.animateTick and BlockMillionFurnace's, from the furnace's client block-entity tick (each loaded
/// copy, drawn or not). Vanilla's `ClientLevel.animateTick` reaches a block 667 times a tick at each of two ranges,
/// each try an offset of `rand(r) - rand(r)` per axis: the chance it lands on a block `d` away is
/// prod((r - |d|) / r^2) per try, so a furnace at the player's feet animates about one tick in five, and none past 32.
pub(crate) fn animate(host: &mut dyn ClientHost, p: Pos) {
    let Some(keys) = KEYS.get() else { return };
    let Some(player) = host.local_player().and_then(|e| e.entity().position(&*host)) else { return };
    let Some(level) = host.level(host.dimension()) else { return };
    let now = with_state(|s| s.client_ticks);
    let d: Vec<i32> = (0..3).map(|i| p[i] - player[i].floor() as i32).collect();
    let reach = |r: i32| d.iter().map(|k| if k.abs() < r { f64::from(r - k.abs()) / f64::from(r * r) } else { 0.0 }).product::<f64>();
    let chance = (667.0 * (reach(16) + reach(32))).min(1.0);
    let mut seed = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |t| t.as_nanos() as u64)
        ^ now.rotate_left(17)
        ^ (p[0] as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (p[2] as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ p[1] as u64)
        | 1;
    {
        if next(&mut seed) >= chance {
            return;
        }
        if level.block_property(&*host, p, "lit") != Some("true") {
            return;
        }
        let kind = level.block_property(&*host, p, "type").unwrap_or_default().to_owned();
        let facing = level.block_property(&*host, p, "facing").unwrap_or_default().to_owned();
        let (x, y, z) = (f64::from(p[0]) + 0.5, f64::from(p[1]), f64::from(p[2]) + 0.5);
        let step = match facing.as_str() {
            "south" => [0.0, 1.0],
            "west" => [-1.0, 0.0],
            "east" => [1.0, 0.0],
            _ => [0.0, -1.0],
        };
        let side = next(&mut seed) * 0.6 - 0.3;
        let (dx, dz) = if step[0] != 0.0 { (step[0] * 0.52, side) } else { (side, step[1] * 0.52) };
        let smoke = ParticleOptions::simple(keys.smoke);
        let flame = ParticleOptions::simple(keys.flame);
        let sound = |id: &str| SoundPlay::at(id, SoundSource::Blocks, [x, y, z]);
        // BlockMillionFurnace: a generating Rainbow Furnace's crits on its four sides.
        if level.block_property(&*host, p, "rainbow") == Some("true") {
            let crit = ParticleOptions::simple(keys.crit);
            for s in [[0.0, -1.0], [0.0, 1.0], [-1.0, 0.0], [1.0, 0.0]] {
                let side = next(&mut seed) * 0.6 - 0.3;
                let (cx, cz) = if s[0] != 0.0 { (s[0] * 0.52, side) } else { (side, s[1] * 0.52) };
                let h = next(&mut seed) * 6.0 / 16.0;
                for _ in 0..10 {
                    let v = [(next(&mut seed) - 0.5) * 0.1, 0.0, (next(&mut seed) - 0.5) * 0.1];
                    host.add_particle(&crit, [x + cx, y + h, z + cz], v);
                }
            }
        }
        match kind.as_str() {
            "1" => {
                if next(&mut seed) < 0.1 {
                    let _ = host.play_local_sound(&sound("minecraft:block.smoker.smoke"), 0);
                }
                host.add_particle(&smoke, [x, y + 1.1, z], [0.0; 3]);
            }
            "2" => {
                if next(&mut seed) < 0.1 {
                    let _ = host.play_local_sound(&sound("minecraft:block.blastfurnace.fire_crackle"), 0);
                }
                host.add_particle(&smoke, [x + dx, y + next(&mut seed) * 9.0 / 16.0, z + dz], [0.0; 3]);
            }
            _ => {
                if next(&mut seed) < 0.1 {
                    let _ = host.play_local_sound(&sound("minecraft:block.furnace.fire_crackle"), 0);
                }
                let h = y + next(&mut seed) * 6.0 / 16.0;
                host.add_particle(&smoke, [x + dx, h, z + dz], [0.0; 3]);
                host.add_particle(&flame, [x + dx, h, z + dz], [0.0; 3]);
            }
        }
    }
}

/// getComparatorInputOverride (getRedstoneSignalFromContainer over the furnace's slots), kept for `analog_output`, which
/// can't read an item's maximum stack size (sunrise-org/sunrise#5473);
/// comparators beside it re-read it when it changes (updateNeighbourForOutputSignal).
pub(crate) fn comparator(host: &mut dyn ServerHost, pos: Pos, block: &str, e: &Fields) {
    let slots = furnace::comparator_slots(host, block, e);
    let (mut total, mut any) = (0.0f32, false);
    for stack in slots.iter().filter_map(|i| e.items.get(*i)).flatten() {
        total += stack.count() as f32 / max_stack(host, stack) as f32;
        any = true;
    }
    total /= slots.len().max(1) as f32;
    let value = (total * 14.0).floor() as i32 + i32::from(any);
    let key = (host.dimension().to_owned(), pos);
    let changed = with_state(|s| s.comparator.insert(key, value) != Some(value));
    if changed
        && let Some(level) = host.level(host.dimension())
        && let Some(state) = level.block_state(host, pos)
    {
        let _ = level.update_neighbors(host, pos, state, NeighbourUpdate::OutputSignal);
    }
}

