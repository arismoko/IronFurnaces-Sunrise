//! PlayerFurnacesList: the furnaces each player placed, kept on the player (an attachment, saved with them, as the
//! Java mod's `PLAYER_FURNACES_LIST` attachment). The Rainbow Furnace looks through its owner's.

use crate::util::*;
use mod_api::*;
use std::sync::RwLock;

fn encode(list: &Vec<Pos>) -> Result<Compound, AttachmentError> {
    let mut flat = Vec::with_capacity(list.len() * 3);
    for p in list {
        flat.extend_from_slice(p);
    }
    let mut c = Compound::new();
    c.insert(java("furnaces"), Tag::IntArray(flat));
    Ok(c)
}

fn decode(c: &Compound) -> Result<Vec<Pos>, AttachmentError> {
    let flat = int_array(c, "furnaces", 0);
    Ok(flat
        .as_chunks::<3>()
        .0
        .iter()
        .map(|p| [p[0], p[1], p[2]])
        .collect())
}

/// The key issued for this registration pass (a reinstall issues a new one).
static FURNACES_LIST: RwLock<Option<AttachmentKey<Vec<Pos>>>> = RwLock::new(None);

/// Declares the attachment: saved with the player and kept through death.
pub(crate) fn register(r: &mut dyn Registrar) {
    let descriptor = AttachmentDescriptor::new("ironfurnaces:player_furnaces_list", Vec::new)
        .serialize(AttachmentCodec { encode, decode })
        .copy_on_death();
    // A refusal is recorded on the load by the registrar itself.
    if let Ok(key) = r.register_attachment_type(descriptor) {
        *FURNACES_LIST.write().unwrap() = Some(key);
    }
}

fn key() -> Option<AttachmentKey<Vec<Pos>>> {
    FURNACES_LIST.read().unwrap().clone()
}

pub(crate) fn add(host: &mut dyn ServerHost, player: u128, pos: Pos) {
    let Some(key) = key() else { return };
    let holder = Holder::Player(player);
    let mut list = key.get(host, holder, Clone::clone).unwrap_or_default();
    if !list.contains(&pos) {
        list.push(pos);
        let _ = key.set(host, holder, list);
    }
}

pub(crate) fn list(host: &mut dyn ServerHost, player: u128) -> Vec<Pos> {
    let Some(key) = key() else { return Vec::new() };
    key.get_existing(host, Holder::Player(player), |list| list.cloned())
        .ok()
        .flatten()
        .unwrap_or_default()
}
