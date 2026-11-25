//! BlockIronFurnaceContainerBase.quickMoveStack: the shift-click's moves, as the Java menu makes them (input or fuel by
//! what the item is, augments to their slots from any page, results into the inventory from its end, everything else
//! between the inventory and the hotbar), and vanilla's loop that asks again while the same item keeps moving.
//!
//! Made in `menu_quick_move`, with the player's state swapped in: the furnace's slots in the lent entity, the player's
//! other slots through `ItemPlace`, and the clicked slot through the hook's `stack`, which it keeps (`None` when all of
//! it moved).

use crate::furnace::{self, Fields, SLOTS};
use crate::util::*;
use crate::{config, recipes};
use mod_api::*;

/// The menu's slots as the Java menu numbers them: the furnace's 19, the main inventory's 27, the hotbar's 9.
const INV: usize = SLOTS;
const HOTBAR: usize = SLOTS + 27;
const END: usize = SLOTS + 36;

/// The player inventory slot (0..9 hotbar, 9..36 main) behind menu slot `i` (from `INV`).
fn player_slot(i: usize) -> i32 {
    let m = i - INV;
    if m < 27 {
        (m + 9) as i32
    } else {
        (m - 27) as i32
    }
}

pub(crate) struct Menu<'a> {
    host: &'a dyn Host,
    level: LevelRef,
    block: String,
    /// The furnace's fields with the model's slots, for its slot rules.
    fields: Fields,
    slots: Vec<Option<ItemStack>>,
}

impl Menu<'_> {
    fn id(&self, s: &ItemStack) -> String {
        id_of(self.host, s)
    }

    /// Slot.mayPlace of the Java menu's slot classes.
    fn may_place(&self, i: usize, s: &ItemStack) -> bool {
        if i >= INV {
            return true;
        }
        let host = self.host;
        let id = self.id(s);
        let mut fields = self.fields.clone();
        fields.items = self.slots[..SLOTS].to_vec();
        match i {
            furnace::INPUT | 7..=12 => furnace::cook_recipe(host, self.level, &fields, s).is_some(),
            furnace::FUEL => furnace::is_fuel(host, s) || id == "minecraft:bucket",
            furnace::AUGMENT_RED..=furnace::AUGMENT_BLUE => furnace::augment_slot(&id) == Some(i),
            furnace::GENERATOR_FUEL => {
                let red = fields.items[furnace::AUGMENT_RED]
                    .as_ref()
                    .map(|r| self.id(r));
                match red.as_deref() {
                    Some("ironfurnaces:augment_blasting") => {
                        recipes::generator_energy(host, s).is_some()
                    }
                    Some("ironfurnaces:augment_smoking") => furnace::smoking_burn(host, s) > 0,
                    _ => !furnace::is_heater_item(&id) && furnace::burn_time(host, s) > 0,
                }
            }
            _ => false,
        }
    }

    /// Slot.getMaxStackSize(stack).
    fn max(&self, i: usize, s: &ItemStack) -> u32 {
        let own = max_stack(self.host, s);
        match i {
            furnace::AUGMENT_RED..=furnace::AUGMENT_BLUE => 1,
            furnace::FUEL if self.id(s) == "minecraft:bucket" => 1,
            _ => own,
        }
    }

    /// AbstractContainerMenu.moveItemStackTo: fills the stacks of the same item, then the first empty slot that takes it.
    fn move_to(
        &mut self,
        stack: &mut Option<ItemStack>,
        start: usize,
        end: usize,
        reverse: bool,
    ) -> bool {
        let order: Vec<usize> = if reverse {
            (start..end).rev().collect()
        } else {
            (start..end).collect()
        };
        let mut moved = false;
        let Some(s0) = stack.clone() else {
            return false;
        };
        if max_stack(self.host, &s0) > 1 {
            for &i in &order {
                let Some(s) = stack.clone() else { break };
                if let Some(have) = self.slots[i].clone()
                    && have.is_stackable_with(&s)
                {
                    let k = self.max(i, &have);
                    let total = have.count() + s.count();
                    if total <= k {
                        self.slots[i] = with_count(&have, total);
                        *stack = None;
                        moved = true;
                    } else if have.count() < k {
                        *stack = with_count(&s, s.count() - (k - have.count()));
                        self.slots[i] = with_count(&have, k);
                        moved = true;
                    }
                }
            }
        }
        if let Some(s) = stack.clone() {
            for &i in &order {
                if self.slots[i].is_none() && self.may_place(i, &s) {
                    let n = self.max(i, &s).min(s.count());
                    self.slots[i] = with_count(&s, n);
                    *stack = if n == s.count() {
                        None
                    } else {
                        with_count(&s, s.count() - n)
                    };
                    moved = true;
                    break;
                }
            }
        }
        moved
    }

    /// specialMoveItemStackTo: as moveItemStackTo, but one item into an empty slot (an item that leaves a remainder).
    fn special_move_to(&mut self, stack: &mut Option<ItemStack>, start: usize, end: usize) -> bool {
        let Some(s) = stack.clone() else { return false };
        let mut moved = false;
        if max_stack(self.host, &s) > 1 {
            for i in start..end {
                let Some(s) = stack.clone() else { break };
                if let Some(have) = self.slots[i].clone()
                    && have.is_stackable_with(&s)
                {
                    let k = self.max(i, &have);
                    let total = have.count() + s.count();
                    if total <= k {
                        self.slots[i] = with_count(&have, total);
                        *stack = None;
                        moved = true;
                    } else if have.count() < k {
                        *stack = with_count(&s, s.count() - (k - have.count()));
                        self.slots[i] = with_count(&have, k);
                        moved = true;
                    }
                }
            }
        }
        if let Some(s) = stack.clone() {
            for i in start..end {
                if self.slots[i].is_none() && self.may_place(i, &s) {
                    self.slots[i] = with_count(&s, 1);
                    *stack = with_count(&s, s.count() - 1).filter(|_| s.count() > 1);
                    moved = true;
                    break;
                }
            }
        }
        moved
    }

    /// The augment branches and the inventory/hotbar fallback shared by the three pages. False: stop (EMPTY).
    fn augment_or_inventory(&mut self, stack: &mut Option<ItemStack>, index: usize) -> bool {
        let Some(s) = stack.clone() else { return true };
        match furnace::augment_slot(&self.id(&s)) {
            Some(slot) => self.move_to(stack, slot, slot + 1, false),
            None if (INV..HOTBAR).contains(&index) => self.move_to(stack, HOTBAR, END, false),
            None if (HOTBAR..END).contains(&index) => self.move_to(stack, INV, HOTBAR, false),
            None => true,
        }
    }

    fn has_recipe(&self, s: &ItemStack) -> bool {
        let mut fields = self.fields.clone();
        fields.items = self.slots[..SLOTS].to_vec();
        furnace::cook_recipe(self.host, self.level, &fields, s).is_some()
    }

    /// quickMoveStack(index): true when something moved.
    fn quick_move(&mut self, index: usize) -> bool {
        let Some(before) = self.slots[index].clone() else {
            return false;
        };
        let mut stack = Some(before.clone());
        let mut fields = self.fields.clone();
        fields.items = self.slots[..SLOTS].to_vec();
        let mode = furnace::mode(self.host, &fields);
        let tier = config::tier(self.host, &self.block);
        let ok = match mode {
            2 => {
                if ![6, 3, 4, 5].contains(&index) {
                    let s = before.clone();
                    let red = fields.items[furnace::AUGMENT_RED]
                        .as_ref()
                        .map(|r| self.id(r));
                    let first = match red.as_deref() {
                        Some("ironfurnaces:augment_smoking")
                            if furnace::smoking_burn(self.host, &s) > 0 =>
                        {
                            if self.host.item_crafting_remainder(&s).is_some() {
                                !(self.slots[6].is_none()
                                    && !self.special_move_to(&mut stack, 6, 7))
                            } else {
                                self.move_to(&mut stack, 6, 7, false)
                            }
                        }
                        Some("ironfurnaces:augment_smoking") => true,
                        Some("ironfurnaces:augment_blasting") => {
                            !(recipes::generator_energy(self.host, &s).is_some()
                                && !self.move_to(&mut stack, 6, 7, false))
                        }
                        _ => {
                            !(furnace::is_fuel(self.host, &s)
                                && !furnace::is_heater_item(&self.id(&s))
                                && !self.move_to(&mut stack, 6, 7, false))
                        }
                    };
                    first && self.augment_or_inventory(&mut stack, index)
                } else {
                    self.move_to(&mut stack, INV, END, false)
                }
            }
            1 => {
                if (13..=18).contains(&index) {
                    self.move_to(&mut stack, INV, END, true)
                } else if index >= INV {
                    if self.has_recipe(&before) {
                        let range = furnace::factory_range(tier);
                        self.move_to(&mut stack, 7 + range.start, 7 + range.end, false)
                    } else {
                        self.augment_or_inventory(&mut stack, index)
                    }
                } else {
                    self.move_to(&mut stack, INV, END, false)
                }
            }
            _ => {
                if index == furnace::OUTPUT {
                    self.move_to(&mut stack, INV, END, true)
                } else if ![0, 1, 3, 4, 5].contains(&index) {
                    if self.has_recipe(&before) {
                        self.move_to(&mut stack, 0, 1, false)
                    } else if furnace::is_fuel(self.host, &before) {
                        self.move_to(&mut stack, 1, 2, false)
                    } else {
                        self.augment_or_inventory(&mut stack, index)
                    }
                } else {
                    self.move_to(&mut stack, INV, END, false)
                }
            }
        };
        let left = stack.as_ref().map_or(0, |s| s.count());
        self.slots[index] = stack;
        ok && left != before.count()
    }
}

/// Makes a shift-click of menu slot `index` from the player's inventory (the hook's `stack`) into the furnace whose
/// fields are lent (`e`), or between the inventory and the hotbar. True when the hook takes the click.
pub(crate) fn run(
    host: &mut dyn ServerHost,
    player: u128,
    level: LevelRef,
    block: &str,
    e: &mut Fields,
    index: usize,
    clicked: &mut Option<ItemStack>,
) -> bool {
    let Some(stack) = clicked.clone() else {
        return false;
    };
    let stack = &stack;
    if !(INV..END).contains(&index) {
        return false;
    }
    if e.items.len() < SLOTS {
        e.items.resize(SLOTS, None);
    }
    let mut slots: Vec<Option<ItemStack>> = e.items[..SLOTS].to_vec();
    for i in INV..END {
        slots.push(if i == index {
            Some(stack.clone())
        } else {
            ItemPlace::PlayerSlot {
                player,
                slot: player_slot(i),
            }
            .with_stack(host, |s| s.project())
            .flatten()
        });
    }
    let original = slots.clone();
    let fields = Fields {
        nbt: e.nbt.clone(),
        items: Vec::new(),
        data: Vec::new(),
    };
    let mut menu = Menu {
        host: &*host,
        level,
        block: block.to_owned(),
        fields,
        slots,
    };
    // AbstractContainerMenu.doClick's QUICK_MOVE: again while it moves and the slot still holds the same item.
    let mut moved = false;
    for _ in 0..64 {
        if !menu.quick_move(index) {
            break;
        }
        moved = true;
        if !matches!(&menu.slots[index], Some(a) if a.same_item(stack)) {
            break;
        }
    }
    if !moved {
        // Nothing moves (Java's quickMoveStack gives EMPTY): the click is taken, nothing changes.
        return true;
    }
    let after = menu.slots;
    // The player's other slots first (what leaves, then what comes), all or nothing.
    let done = host.transaction(|host, tx| {
        for pass in 0..2 {
            for i in INV..END {
                if i == index || after[i] == original[i] {
                    continue;
                }
                let place = ItemPlace::PlayerSlot {
                    player,
                    slot: player_slot(i),
                };
                let (side, out) = if pass == 0 {
                    (&original[i], true)
                } else {
                    (&after[i], false)
                };
                if let Some(s) = side {
                    let n = if out {
                        place.extract(host, tx, &ItemResource::of(s), s.count())
                    } else {
                        place.insert(host, tx, &ItemResource::of(s), s.count())
                    }
                    .unwrap_or(0);
                    if n != s.count() {
                        return Err(());
                    }
                }
            }
        }
        Ok(())
    });
    if done.is_err() {
        return true;
    }
    for i in 0..SLOTS {
        if after[i] != original[i] {
            e.items[i] = after[i].clone();
        }
    }
    *clicked = after[index].clone();
    true
}
