//! The furnaces' item and energy capabilities (NeoForge's `Capabilities.Item.BLOCK` with `WorldlyContainerWrapper`,
//! and `Capabilities.Energy.BLOCK` with the furnace's `FEnergyStorage`), answered before the game's own so hoppers,
//! pipes and other mods' machines see what the Java mod's furnaces show them:
//! - items: a side reaches the slots its setting gives (IgetSlotsForFace), takes only what fits there
//!   (IisItemValidForSlot) and gives only what IcanExtractItem allows; no side reaches every slot;
//! - energy: a factory takes energy and gives none, a generator gives and takes none, a furnace neither (the Java tick
//!   sets the storage's maxReceive and maxExtract so).

use crate::config;
use crate::furnace::{self, Fields, SLOTS};
use crate::util::*;
use mod_api::*;

/// The furnace at `at`: its block and its fields (saved compound and slots; no menu data).
fn read(host: &dyn Host, at: &BlockSide) -> Option<(String, BlockEntityRef, Fields)> {
    let block = at.level.block(host, at.pos)?.id(host)?.to_owned();
    if !furnace::is_furnace(&block) {
        return None;
    }
    let entity = host.block_entity(at.level, at.pos)?;
    let nbt = entity.data(host)?;
    let items = (0..SLOTS as u32).map(|i| entity.item_slot(host, i).ok().flatten()).collect();
    Some((block, entity, Fields { nbt, items, data: Vec::new() }))
}

pub(crate) struct FurnaceItems(pub BlockSide);

impl FurnaceItems {
    fn setting(&self, e: &Fields) -> Option<i32> {
        self.0.side.map(|d| furnace::side(&e.nbt, dir_index(d)))
    }

    /// The container slot behind handler index `index`.
    fn slot(&self, host: &dyn Host, e: &Fields, index: usize) -> Option<usize> {
        match self.setting(e) {
            Some(setting) => furnace::slots_for_face(host, e, setting).get(index).copied(),
            None => (index < SLOTS).then_some(index),
        }
    }
}

impl ResourceHandler<ItemResource> for FurnaceItems {
    fn size(&self, host: &mut dyn ServerHost) -> usize {
        let Some((_, _, e)) = read(host, &self.0) else { return 0 };
        match self.setting(&e) {
            Some(setting) => furnace::slots_for_face(host, &e, setting).len(),
            None => SLOTS,
        }
    }

    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        let (_, _, e) = read(host, &self.0)?;
        let slot = self.slot(host, &e, index)?;
        e.items[slot].as_ref().map(ItemResource::of)
    }

    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        let Some((_, _, e)) = read(host, &self.0) else { return 0 };
        self.slot(host, &e, index).and_then(|s| e.items[s].as_ref()).map_or(0, |s| s.count())
    }

    fn capacity(&self, host: &mut dyn ServerHost, index: usize, resource: Option<&ItemResource>) -> u32 {
        let Some((block, _, e)) = read(host, &self.0) else { return 0 };
        let Some(slot) = self.slot(host, &e, index) else { return 0 };
        // The general limit: the container's (`Container.getMaxStackSize`).
        let Some(resource) = resource else { return 64 };
        if !furnace::valid_for_slot(host, self.0.level, &block, &e, slot, resource.as_stack()) {
            return 0;
        }
        max_stack(host, resource.as_stack())
    }

    fn insert_at(&self, host: &mut dyn ServerHost, tx: &TransactionContext, index: usize, resource: &ItemResource, amount: u32) -> Result<u32, TransactionError<Overanswered>> {
        let Some((block, entity, e)) = read(host, &self.0) else { return Ok(0) };
        let Some(slot) = self.slot(host, &e, index) else { return Ok(0) };
        let stack = resource.as_stack();
        if amount == 0 || !furnace::valid_for_slot(host, self.0.level, &block, &e, slot, stack) {
            return Ok(0);
        }
        let limit = max_stack(host, stack);
        let (have, room) = match e.items[slot].as_ref() {
            Some(h) if !h.is_stackable_with(stack) => return Ok(0),
            Some(h) => (h.count(), limit.saturating_sub(h.count())),
            None => (0, limit),
        };
        let n = room.min(amount);
        if n == 0 {
            return Ok(0);
        }
        let Some(next) = resource.to_stack(have + n) else { return Ok(0) };
        match entity.set_item_slot(host, tx, slot as u32, Some(next)) {
            Ok(true) => Ok(n),
            _ => Ok(0),
        }
    }

    fn extract_at(&self, host: &mut dyn ServerHost, tx: &TransactionContext, index: usize, resource: &ItemResource, amount: u32) -> Result<u32, TransactionError<Overanswered>> {
        let Some((_, entity, e)) = read(host, &self.0) else { return Ok(0) };
        let Some(slot) = self.slot(host, &e, index) else { return Ok(0) };
        let Some(held) = e.items[slot].as_ref() else { return Ok(0) };
        if !resource.matches(held) {
            return Ok(0);
        }
        if let Some(setting) = self.setting(&e)
            && !furnace::can_extract(host, &e, slot, held, setting)
        {
            return Ok(0);
        }
        let n = held.count().min(amount);
        if n == 0 {
            return Ok(0);
        }
        let rest = with_count(held, held.count() - n);
        let rest = if held.count() == n { None } else { rest };
        match entity.set_item_slot(host, tx, slot as u32, rest) {
            Ok(true) => Ok(n),
            _ => Ok(0),
        }
    }
}

pub(crate) struct FurnaceEnergy(pub BlockSide);

impl FurnaceEnergy {
    /// (block, entity, stored, mode).
    fn read(&self, host: &dyn Host) -> Option<(String, BlockEntityRef, i32, i32)> {
        let (block, entity, e) = read(host, &self.0)?;
        Some((block, entity, furnace::energy(&e), furnace::mode(host, &e)))
    }

    fn store(host: &mut dyn ServerHost, entity: BlockEntityRef, value: i32) -> bool {
        let mut c = Compound::new();
        set_i(&mut c, "energy", value);
        entity.merge_data(host, c).is_ok()
    }
}

impl EnergyHandler for FurnaceEnergy {
    fn amount(&self, host: &mut dyn ServerHost) -> u32 {
        self.read(host).map_or(0, |(_, _, stored, _)| stored.max(0) as u32)
    }

    fn capacity(&self, host: &mut dyn ServerHost) -> u32 {
        self.read(host).map_or(0, |(block, ..)| config::capacity(host, &block).max(0) as u32)
    }

    fn insert(&self, host: &mut dyn ServerHost, tx: &TransactionContext, amount: u32) -> Result<u32, TransactionError<Overanswered>> {
        let _ = tx;
        let Some((block, entity, stored, mode)) = self.read(host) else { return Ok(0) };
        if mode != 1 {
            return Ok(0);
        }
        let capacity = config::capacity(host, &block);
        let n = (capacity - stored).max(0).min(amount.min(i32::MAX as u32) as i32);
        Ok(if n > 0 && Self::store(host, entity, stored + n) { n as u32 } else { 0 })
    }

    fn extract(&self, host: &mut dyn ServerHost, tx: &TransactionContext, amount: u32) -> Result<u32, TransactionError<Overanswered>> {
        let _ = tx;
        let Some((_, entity, stored, mode)) = self.read(host) else { return Ok(0) };
        if mode != 2 {
            return Ok(0);
        }
        let n = stored.max(0).min(amount.min(i32::MAX as u32) as i32);
        Ok(if n > 0 && Self::store(host, entity, stored - n) { n as u32 } else { 0 })
    }
}
