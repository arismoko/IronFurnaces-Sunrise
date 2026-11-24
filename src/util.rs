//! Small helpers: the saved compound's typed reads and writes, items by name, block positions.

use mod_api::item::containers::ModBlockEntityFields;
use mod_api::*;
use std::collections::HashMap;
use std::sync::OnceLock;

pub(crate) type Pos = [i32; 3];

pub(crate) fn java(s: &str) -> JavaString {
    JavaString::from(s)
}

pub(crate) fn get_i(nbt: &Compound, key: &str, default: i32) -> i32 {
    match nbt.get(&java(key)) {
        Some(Tag::Int(v)) => *v,
        _ => default,
    }
}

pub(crate) fn set_i(nbt: &mut Compound, key: &str, value: i32) {
    if get_i(nbt, key, value.wrapping_add(1)) != value {
        nbt.insert(java(key), Tag::Int(value));
    }
}

pub(crate) fn get_f(nbt: &Compound, key: &str) -> f64 {
    match nbt.get(&java(key)) {
        Some(Tag::Double(bits)) => f64::from_bits(*bits),
        _ => 0.0,
    }
}

pub(crate) fn set_f(nbt: &mut Compound, key: &str, value: f64) {
    if !matches!(nbt.get(&java(key)), Some(Tag::Double(bits)) if *bits == value.to_bits()) {
        nbt.insert(java(key), Tag::Double(value.to_bits()));
    }
}

pub(crate) fn int_array(nbt: &Compound, key: &str, len: usize) -> Vec<i32> {
    let mut out = match nbt.get(&java(key)) {
        Some(Tag::IntArray(a)) => a.to_vec(),
        _ => Vec::new(),
    };
    out.resize(len, 0);
    out
}

pub(crate) fn set_int_array(nbt: &mut Compound, key: &str, values: Vec<i32>) {
    if int_array(nbt, key, values.len()) != values || !matches!(nbt.get(&java(key)), Some(Tag::IntArray(_))) {
        nbt.insert(java(key), Tag::IntArray(values));
    }
}

/// Item keys by id, looked up once (keys stay for the process).
fn item_ids(host: &dyn Host) -> &'static HashMap<String, ItemKey> {
    static IDS: OnceLock<HashMap<String, ItemKey>> = OnceLock::new();
    IDS.get_or_init(|| host.item_keys().into_iter().filter_map(|k| Some((k.id(host)?.to_owned(), k))).collect())
}

pub(crate) fn item_key(host: &dyn Host, id: &str) -> Option<ItemKey> {
    item_ids(host).get(id).copied()
}

/// `count` of item `id`.
pub(crate) fn stack_of(host: &dyn Host, id: &str, count: u32) -> Option<ItemStack> {
    ItemStack::new(item_key(host, id)?, count)
}

pub(crate) fn id_of(host: &dyn Host, stack: &ItemStack) -> String {
    stack.item().id(host).unwrap_or_default().to_owned()
}

pub(crate) fn is(host: &dyn Host, stack: &Option<ItemStack>, id: &str) -> bool {
    stack.as_ref().is_some_and(|s| id_of(host, s) == id)
}

/// The item's own stack limit (`getMaxStackSize`), at most 64 as a container's.
pub(crate) fn max_stack(host: &dyn Host, stack: &ItemStack) -> u32 {
    host.max_stack_size(stack.item()).unwrap_or(64).clamp(1, 64)
}

/// `stack` with `count`, none for 0.
pub(crate) fn with_count(stack: &ItemStack, count: u32) -> Option<ItemStack> {
    let mut out = stack.clone();
    out.set_count(count).ok()?;
    Some(out)
}

pub(crate) fn shrink(slot: &mut Option<ItemStack>, by: u32) {
    if let Some(s) = slot.take() {
        *slot = s.shrink(by);
    }
}

pub(crate) const DIRECTIONS: [Direction; 6] =
    [Direction::Down, Direction::Up, Direction::North, Direction::South, Direction::West, Direction::East];

pub(crate) fn dir_index(d: Direction) -> usize {
    DIRECTIONS.iter().position(|x| *x == d).unwrap_or(0)
}

pub(crate) fn step(i: usize) -> Pos {
    [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]][i]
}

pub(crate) fn opposite(i: usize) -> usize {
    [1, 0, 3, 2, 5, 4][i]
}

pub(crate) fn offset(pos: Pos, dir: usize) -> Pos {
    let s = step(dir);
    [pos[0] + s[0], pos[1] + s[1], pos[2] + s[2]]
}

pub(crate) fn centre(pos: Pos) -> [f64; 3] {
    [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5]
}

pub(crate) fn lang(host: &dyn ClientHost, key: &str, english: &str) -> String {
    host.translate(key).unwrap_or_else(|| english.to_owned())
}

/// A compound in the stack's `minecraft:custom_data` component.
pub(crate) fn custom_data(stack: &ItemStack) -> Option<Compound> {
    match stack.components()?.as_nbt().get(&java("minecraft:custom_data")) {
        Some(Tag::Compound(c)) => Some(c.clone()),
        _ => None,
    }
}

pub(crate) fn set_custom_data(stack: &mut ItemStack, data: Compound) {
    let mut patch: Compound = stack.components().map(Compound::from).unwrap_or_default();
    patch.insert(java("minecraft:custom_data"), Tag::Compound(data));
    let _ = stack.set_components(patch);
}

pub(crate) fn sneaking(host: &dyn Host, player: PlayerRef) -> bool {
    let entity = player.entity();
    entity.is_shift_key_down(host).unwrap_or(false) || entity.is_crouching(host).unwrap_or(false)
}

/// A slot of a screen at `position` (GUI pixels from the screen's corner), with no empty-slot icon.
pub(crate) fn slot_at(position: [f32; 2]) -> ScreenSlot {
    ScreenSlot { position, no_item_icon: None }
}

/// A block entity tick on an owned copy of its fields: the saved compound, the item slots and the menu data, as the
/// tick hook handed them before it lent the resident entity in place. `tick` runs with the host and the copy; what
/// it changed is written back to the entity and the chunk is marked to save (`setChanged`). A block entity gone
/// between the two loans keeps nothing.
pub(crate) fn tick_on_fields(
    subject: &mut BlockEntityScope<'_>,
    tick: impl FnOnce(&mut dyn ServerHost, Pos, &mut ModBlockEntityFields),
) {
    let Some((handle, pos, before)) = subject.with_view(|view| (view.handle(), view.position(), fields_of(&view))) else {
        return;
    };
    let mut after = before.clone();
    tick(subject.host(), pos, &mut after);
    if after == before {
        return;
    }
    if subject.with_view(|mut view| write_fields(&mut view, &before, &after)) == Some(true) {
        let _ = handle.set_changed(subject.host());
    }
}

/// The entity's fields, copied out of the view.
pub(crate) fn fields_of(view: &BlockEntityView<'_>) -> ModBlockEntityFields {
    ModBlockEntityFields {
        nbt: view.nbt().clone(),
        items: (0..view.item_slots() as u32).map(|i| view.item_slot(i).and_then(|s| s.to_owned())).collect(),
        data: view.data_slots().to_vec(),
    }
}

/// Writes what changed from `before` to `after` into the view: true when anything was written.
pub(crate) fn write_fields(view: &mut BlockEntityView<'_>, before: &ModBlockEntityFields, after: &ModBlockEntityFields) -> bool {
    let mut wrote = false;
    for (key, value) in &after.nbt {
        if before.nbt.get(key) != Some(value) {
            wrote |= view.set_nbt(key.clone(), Some(value.clone()));
        }
    }
    for key in before.nbt.keys() {
        if !after.nbt.contains_key(key) {
            wrote |= view.set_nbt(key.clone(), None);
        }
    }
    for (slot, stack) in after.items.iter().enumerate().take(view.item_slots()) {
        if before.items.get(slot) != Some(stack) {
            wrote |= view.set_item_slot(slot as u32, stack.clone()).unwrap_or(false);
        }
    }
    if after.data != before.data {
        if after.data.len() == before.data.len() {
            for (slot, value) in after.data.iter().enumerate() {
                if before.data[slot] != *value {
                    wrote |= view.set_data_slot(slot as u32, *value);
                }
            }
        } else {
            wrote |= view.set_data_slots(&after.data);
        }
    }
    wrote
}
